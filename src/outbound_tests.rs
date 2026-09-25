//! Wire-проверки собственного outbound SMTP transport против отдельного peer.
use crate::outbound::{self, Connection, DeliveryOutcome, TransportLimits};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

#[test]
fn peer_450_becomes_deferred() {
    assert_eq!(
        run_rejecting_peer("450 4.2.0 busy\r\n"),
        DeliveryOutcome::Deferred
    );
}

#[test]
fn peer_550_becomes_permanent_failure() {
    assert_eq!(
        run_rejecting_peer("550 5.1.1 unknown\r\n"),
        DeliveryOutcome::PermanentFailure
    );
}

#[test]
fn stalled_greeting_hits_io_deadline_and_drops_the_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut byte = [0_u8; 1];
        assert_eq!(stream.read(&mut byte).expect("client close"), 0);
    });
    let limits = TransportLimits::testing(Duration::from_secs(1), Duration::from_millis(100));
    let error = match Connection::connect_with_limits(&address.to_string(), limits) {
        Ok(_) => panic!("silent peer must time out"),
        Err(error) => error,
    };
    assert!(matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ));
    peer.join().expect("peer");
}

#[test]
fn route_rejects_hostname_without_starting_hidden_dns() {
    let error = match Connection::connect("peer.example:25") {
        Ok(_) => panic!("numeric route required"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn smtp_data_terminator_does_not_add_an_empty_body_line() {
    let mut wire = Vec::new();
    outbound::write_data(&mut wire, b"body\r\n").expect("DATA write");
    assert_eq!(wire, b"body\r\n.\r\n");
}

#[test]
fn smtp_data_stuffs_only_lines_that_begin_with_a_dot() {
    let mut wire = Vec::new();
    outbound::write_data(&mut wire, b"header\r\n\r\n.\r\n..text\r\n").expect("DATA write");
    assert_eq!(wire, b"header\r\n\r\n..\r\n...text\r\n.\r\n");
}

#[test]
fn outbound_data_enforces_line_octet_limit_and_crlf_framing() {
    let mut maximum_line = vec![b'x'; 998];
    maximum_line.extend_from_slice(b"\r\n");
    assert_eq!(outbound::validate_data(&maximum_line), Ok(()));

    let mut oversized_line = vec![b'x'; 999];
    oversized_line.extend_from_slice(b"\r\n");
    assert_eq!(
        outbound::validate_data(&oversized_line),
        Err(outbound::DataValidationError::LineTooLong)
    );
    assert_eq!(
        outbound::validate_data(b"bare\nline"),
        Err(outbound::DataValidationError::BareLineFeed)
    );
    assert_eq!(
        outbound::validate_data(b"bare\rline"),
        Err(outbound::DataValidationError::BareCarriageReturn)
    );
}

#[test]
fn outbound_8bit_profile_distinguishes_mime_body_from_headers() {
    let body = "Subject: encoded header\r\n\r\nПривет".as_bytes();
    assert_eq!(outbound::validate_data(body), Ok(()));
    assert!(outbound::has_8bit_body(body));
    assert!(!outbound::has_8bit_body(b"Subject: plain\r\n\r\nbody"));

    let utf8_header = "Subject: Привет\r\n\r\nbody".as_bytes();
    assert_eq!(
        outbound::validate_data(utf8_header),
        Err(outbound::DataValidationError::NonAsciiHeader)
    );
}

fn run_rejecting_peer(reply: &str) -> DeliveryOutcome {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let reply = reply.to_owned();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        read_command(&mut reader, "EHLO");
        stream.write_all(b"250-peer\r\n250 ok\r\n").expect("ehlo");
        read_command(&mut reader, "MAIL FROM:");
        stream.write_all(b"250 ok\r\n").expect("mail");
        read_command(&mut reader, "RCPT TO:");
        stream.write_all(reply.as_bytes()).expect("reject");
    });
    let outcome = outbound::deliver(&address.to_string(), "alice@test", "remote@test", b"body")
        .expect("transport");
    peer.join().expect("peer");
    outcome
}

fn read_command(reader: &mut BufReader<std::net::TcpStream>, prefix: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("command");
    assert!(line.starts_with(prefix), "unexpected command: {line}");
}
