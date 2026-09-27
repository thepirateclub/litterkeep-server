//! Независимая проверка отказа relay перед SMTP MAIL без BINARYMIME.
use crate::outbound::{Connection, DeliveryOutcome};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

/// Peer объявляет CHUNKING и 8BITMIME, но не BINARYMIME.
#[test]
fn binary_cte_is_not_sent_to_peer_without_binarymime() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("SMTP peer listener");
    let address = listener.local_addr().expect("SMTP peer address");
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept outbound client");
        verify_capabilities_without_binarymime(stream);
    });
    let message = b"MIME-Version: 1.0\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xffpayload";

    let outcome = {
        let mut connection = Connection::connect(&address.to_string()).expect("connect peer");
        connection
            .deliver("alice@test", "remote@test", message)
            .expect("permanent routing result")
    };

    assert_eq!(outcome, DeliveryOutcome::PermanentFailure);
    peer.join().expect("peer assertion");
}

/// Подтверждает capability dialogue и отсутствие MAIL после отказа.
fn verify_capabilities_without_binarymime(mut stream: TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("peer read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone peer socket"));
    stream
        .write_all(b"220 peer ready\r\n")
        .expect("SMTP greeting");
    expect_line(&mut reader, "EHLO litterkeep.test\r\n");
    stream
        .write_all(b"250-peer.test\r\n250-CHUNKING\r\n250-8BITMIME\r\n250 SIZE 1024\r\n")
        .expect("EHLO capabilities");
    let mut unexpected = String::new();
    let read = reader.read_line(&mut unexpected).expect("read after EHLO");
    assert_eq!(read, 0, "client must close without sending MAIL");
}

/// Сверяет SMTP command целиком, не только его verb.
fn expect_line(reader: &mut BufReader<TcpStream>, expected: &str) {
    let mut actual = String::new();
    reader.read_line(&mut actual).expect("read SMTP command");
    assert_eq!(actual, expected);
}
