//! Проверка RFC SMTP QUIT reply timeout после подтверждённой доставки.

use super::{Connection, DeliveryOutcome, TransportLimits};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

/// Отсутствующий 221 на QUIT ограничен command timeout и не меняет DATA result.
#[test]
fn quit_reply_timeout_is_bounded_after_message_acceptance() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("SMTP test listener");
    let address = listener.local_addr().expect("listener address");
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept outbound connection");
        serve_without_quit_reply(stream);
    });
    let limits = TransportLimits::testing(Duration::from_secs(1), Duration::from_millis(100));
    let (delivery, quit) = {
        let mut connection = Connection::connect_with_limits(&address.to_string(), limits)
            .expect("connect SMTP peer");
        let message = b"Subject: timeout check\r\n\r\nbody\r\n";
        let delivery = connection
            .deliver("alice@test", "remote@test", message)
            .expect("DATA delivery result");
        (delivery, connection.quit())
    };

    assert!(matches!(delivery, DeliveryOutcome::Delivered));
    let error = quit.expect_err("peer omits QUIT 221");
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ));
    peer.join().expect("peer observed bounded client close");
}

fn serve_without_quit_reply(mut stream: TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("peer read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone peer stream"));
    stream.write_all(b"220 peer ready\r\n").expect("greeting");
    expect_line(&mut reader, b"EHLO litterkeep.test\r\n");
    stream.write_all(b"250 peer.test\r\n").expect("EHLO reply");
    expect_line(&mut reader, b"MAIL FROM:<alice@test>\r\n");
    stream.write_all(b"250 sender\r\n").expect("MAIL reply");
    expect_line(&mut reader, b"RCPT TO:<remote@test>\r\n");
    stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
    expect_line(&mut reader, b"DATA\r\n");
    stream.write_all(b"354 send data\r\n").expect("DATA reply");
    read_data(&mut reader);
    stream
        .write_all(b"250 accepted\r\n")
        .expect("message accepted");
    expect_line(&mut reader, b"QUIT\r\n");
    thread::sleep(Duration::from_millis(250));
    let mut extra = String::new();
    assert_eq!(reader.read_line(&mut extra).expect("client close"), 0);
}

fn read_data(reader: &mut BufReader<TcpStream>) {
    loop {
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line).expect("DATA line");
        if line.as_slice() == b".\r\n" {
            return;
        }
    }
}

fn expect_line(reader: &mut BufReader<TcpStream>, expected: &[u8]) {
    let mut actual = Vec::new();
    reader.read_until(b'\n', &mut actual).expect("SMTP command");
    assert_eq!(actual, expected);
}
