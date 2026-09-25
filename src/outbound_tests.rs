//! Wire-проверки собственного outbound SMTP transport против отдельного peer.
use crate::outbound::{self, Connection, DeliveryOutcome, TransportLimits};
use std::{
    io::{self, BufRead, BufReader, Cursor, Read, Write},
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
fn outbound_falls_back_to_helo_only_when_ehlo_is_unrecognized() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        read_command(&mut reader, "EHLO");
        stream
            .write_all(b"500 command not recognized\r\n")
            .expect("EHLO reply");
        read_command_exact(&mut reader, "HELO litterkeep.test\r\n");
        stream.write_all(b"250 hello\r\n").expect("HELO reply");
        read_command_exact(&mut reader, "MAIL FROM:<alice@test>\r\n");
        stream.write_all(b"250 sender\r\n").expect("MAIL reply");
        read_command(&mut reader, "RCPT TO:");
        stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
        read_command_exact(&mut reader, "DATA\r\n");
        stream.write_all(b"354 data\r\n").expect("DATA reply");
        read_command_exact(&mut reader, "body\r\n");
        read_command_exact(&mut reader, ".\r\n");
        stream
            .write_all(b"250 delivered\r\n")
            .expect("delivery reply");
        read_command_exact(&mut reader, "QUIT\r\n");
        stream.write_all(b"221 bye\r\n").expect("QUIT reply");
    });
    assert_eq!(
        outbound::deliver(&address.to_string(), "alice@test", "bob@test", b"body")
            .expect("delivery after HELO fallback"),
        DeliveryOutcome::Delivered
    );
    peer.join().expect("peer");
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
fn smtp_data_streams_bounded_lines_through_partial_writes() {
    let line = vec![b'x'; 997];
    let mut message = Vec::new();
    let mut expected = Vec::new();
    for _ in 0..2048 {
        message.extend_from_slice(b".");
        message.extend_from_slice(&line);
        message.extend_from_slice(b"\r\n");
        expected.extend_from_slice(b"..");
        expected.extend_from_slice(&line);
        expected.extend_from_slice(b"\r\n");
    }
    expected.extend_from_slice(b".\r\n");
    let mut writer = PartialWriter::new(7);
    outbound::write_data(&mut writer, &message).expect("stream DATA");
    assert_eq!(writer.bytes, expected);
    assert!(writer.largest_write <= 1000);
}

/// Writer теста моделирует socket, который принимает только часть буфера.
struct PartialWriter {
    bytes: Vec<u8>,
    chunk_limit: usize,
    largest_write: usize,
}

impl PartialWriter {
    fn new(chunk_limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            chunk_limit,
            largest_write: 0,
        }
    }
}

impl Write for PartialWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.largest_write = self.largest_write.max(buffer.len());
        let count = buffer.len().min(self.chunk_limit);
        self.bytes
            .extend_from_slice(buffer.get(..count).expect("bounded write"));
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
    assert_eq!(outbound::validate_data(utf8_header), Ok(()));
    assert!(outbound::has_utf8_headers(utf8_header));
    assert_eq!(
        outbound::validate_data("Заголовок: value\r\n\r\nbody".as_bytes()),
        Err(outbound::DataValidationError::InvalidUtf8Headers)
    );
}

#[test]
fn smtp_positive_reply_codes_are_specific_to_the_command_phase() {
    let mut mail_continuation = Cursor::new(b"354 send data\r\n");
    assert_eq!(
        outbound::expect(&mut mail_continuation, outbound::ReplyExpectation::Mail)
            .expect_err("MAIL cannot accept DATA continuation")
            .kind(),
        io::ErrorKind::InvalidData
    );

    let mut recipient_forwarded = Cursor::new(b"251 user not local\r\n");
    assert_eq!(
        outbound::expect(
            &mut recipient_forwarded,
            outbound::ReplyExpectation::Recipient
        )
        .expect("forwarded recipient"),
        outbound::ReplyResult::Accepted
    );

    let mut data_continuation = Cursor::new(b"354 send data\r\n");
    assert_eq!(
        outbound::expect(&mut data_continuation, outbound::ReplyExpectation::Data)
            .expect("DATA continuation"),
        outbound::ReplyResult::Accepted
    );

    let mut future_positive_code = Cursor::new(b"299 extension success\r\n");
    assert_eq!(
        outbound::expect(&mut future_positive_code, outbound::ReplyExpectation::Mail)
            .expect("unknown 2yz is a positive completion"),
        outbound::ReplyResult::Accepted
    );
}

#[test]
fn smtp_421_reply_marks_the_connection_as_closing_and_deferred() {
    let mut reply = Cursor::new(b"421 peer shutting down\r\n");
    assert_eq!(
        outbound::expect(&mut reply, outbound::ReplyExpectation::Recipient).expect("421 reply"),
        outbound::ReplyResult::Closing
    );
}

#[test]
fn smtp_multiline_reply_requires_one_status_code_on_every_line() {
    let mut reply = Cursor::new(b"250-first line\r\n550 final line\r\n");
    assert_eq!(
        outbound::expect(&mut reply, outbound::ReplyExpectation::Mail)
            .expect_err("mixed multiline status")
            .kind(),
        io::ErrorKind::InvalidData
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

fn read_command_exact(reader: &mut BufReader<std::net::TcpStream>, expected: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("command");
    assert_eq!(line, expected);
}
