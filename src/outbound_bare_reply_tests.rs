//! Wire-проверка исходящей SMTP доставки с bare reply codes по RFC 5321.
use crate::outbound::{self, DeliveryOutcome};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

/// SMTP client принимает reply codes без текста во всех рабочих фазах.
#[test]
fn delivers_when_peer_omits_reply_text() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind peer");
    let address = listener.local_addr().expect("peer address");
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("bound peer read");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("bound peer write");
        stream.write_all(b"220\r\n").expect("bare greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        read_command(&mut reader, "EHLO ");
        stream
            .write_all(b"250-peer\r\n250\r\n")
            .expect("bare EHLO final");
        read_command(&mut reader, "MAIL FROM:");
        stream.write_all(b"250\r\n").expect("bare MAIL reply");
        read_command(&mut reader, "RCPT TO:");
        stream
            .write_all(b"250 \r\n")
            .expect("empty-text RCPT reply");
        read_command(&mut reader, "DATA");
        stream.write_all(b"354\r\n").expect("bare DATA reply");
        read_command(&mut reader, "body");
        read_command(&mut reader, ".");
        stream.write_all(b"250\r\n").expect("bare completion");
        read_command(&mut reader, "QUIT");
        stream.write_all(b"221\r\n").expect("bare QUIT reply");
    });

    let result = outbound::deliver(&address.to_string(), "alice@test", "bob@test", b"body")
        .expect("outbound delivery");
    assert_eq!(result, DeliveryOutcome::Delivered);
    peer.join().expect("peer thread");
}

/// Считывает команду SMTP и проверяет её начало без индексации буфера.
fn read_command(reader: &mut BufReader<std::net::TcpStream>, prefix: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read command");
    assert!(
        line.starts_with(prefix),
        "unexpected SMTP command: {line:?}"
    );
}
