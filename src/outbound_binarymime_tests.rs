//! Независимая проверка отказа relay перед SMTP MAIL без BINARYMIME.
use crate::outbound::bdat::binary_chunks;
use crate::outbound::{Connection, DeliveryOutcome};
use std::io::{BufRead, BufReader, Read, Write};
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

/// Согласованные CHUNKING+BINARYMIME доставляют произвольные octets через BDAT.
#[test]
fn binary_cte_uses_exact_sequential_bdat_chunks() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("SMTP peer listener");
    let address = listener.local_addr().expect("SMTP peer address");
    let mut message = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xff\r\n\n\r".to_vec();
    message.extend(std::iter::repeat_n(0xA5, 70_000));
    let peer_message = message.clone();
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept outbound client");
        verify_binarymime_delivery(stream, &peer_message);
    });

    let outcome = {
        let mut connection = Connection::connect(&address.to_string()).expect("connect peer");
        connection
            .deliver("alice@test", "remote@test", &message)
            .expect("BINARYMIME delivery")
    };

    assert_eq!(outcome, DeliveryOutcome::Delivered);
    peer.join().expect("peer byte assertions");
}

/// После 4yz на chunk клиент останавливает передачу и не посылает остаток.
#[test]
fn binary_sender_stops_after_transient_chunk_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("SMTP peer listener");
    let address = listener.local_addr().expect("SMTP peer address");
    let mut message =
        b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n"
            .to_vec();
    message.extend(std::iter::repeat_n(0xA5, 70_000));
    let peer_message = message.clone();
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept outbound client");
        verify_first_chunk_failure(stream, &peer_message);
    });

    let outcome = {
        let mut connection = Connection::connect(&address.to_string()).expect("connect peer");
        connection
            .deliver("alice@test", "remote@test", &message)
            .expect("transient queue result")
    };

    assert_eq!(outcome, DeliveryOutcome::Deferred);
    peer.join().expect("no later chunk assertion");
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

/// Проверяет MAIL BODY, точные command counts, каждый ACK и исходные байты.
fn verify_binarymime_delivery(mut stream: TcpStream, message: &[u8]) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("peer read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone peer socket"));
    stream
        .write_all(b"220 peer ready\r\n")
        .expect("SMTP greeting");
    expect_line(&mut reader, "EHLO litterkeep.test\r\n");
    stream
        .write_all(b"250-peer.test\r\n250-BINARYMIME\r\n250-CHUNKING\r\n250 SIZE 1000000\r\n")
        .expect("EHLO capabilities");
    let mail = format!(
        "MAIL FROM:<alice@test> SIZE={} BODY=BINARYMIME\r\n",
        message.len()
    );
    expect_line(&mut reader, &mail);
    stream.write_all(b"250 sender\r\n").expect("MAIL reply");
    expect_line(&mut reader, "RCPT TO:<remote@test>\r\n");
    stream.write_all(b"250 recipient\r\n").expect("RCPT reply");

    for chunk in binary_chunks(message) {
        let marker = match chunk.last {
            true => " LAST",
            false => "",
        };
        expect_line(
            &mut reader,
            &format!("BDAT {}{}\r\n", chunk.bytes.len(), marker),
        );
        let mut payload = vec![0; chunk.bytes.len()];
        reader
            .read_exact(&mut payload)
            .expect("read exact BDAT bytes");
        assert_eq!(payload, chunk.bytes);
        stream
            .write_all(b"250 chunk accepted\r\n")
            .expect("BDAT ack");
    }
    let mut extra = String::new();
    assert_eq!(reader.read_line(&mut extra).expect("no DATA fallback"), 0);
}

/// Принимает первый чанк, отвечает 451 и убеждается, что следующий не отправлен.
fn verify_first_chunk_failure(mut stream: TcpStream, message: &[u8]) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("peer read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone peer socket"));
    stream
        .write_all(b"220 peer ready\r\n")
        .expect("SMTP greeting");
    expect_line(&mut reader, "EHLO litterkeep.test\r\n");
    stream
        .write_all(b"250-peer.test\r\n250-BINARYMIME\r\n250-CHUNKING\r\n250 SIZE 1000000\r\n")
        .expect("EHLO capabilities");
    let mail = format!(
        "MAIL FROM:<alice@test> SIZE={} BODY=BINARYMIME\r\n",
        message.len()
    );
    expect_line(&mut reader, &mail);
    stream.write_all(b"250 sender\r\n").expect("MAIL reply");
    expect_line(&mut reader, "RCPT TO:<remote@test>\r\n");
    stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
    expect_line(
        &mut reader,
        &format!("BDAT {}\r\n", crate::outbound::bdat::MAX_CHUNK_SIZE),
    );
    let mut first_chunk = vec![0; crate::outbound::bdat::MAX_CHUNK_SIZE];
    reader
        .read_exact(&mut first_chunk)
        .expect("read first BDAT chunk");
    stream.write_all(b"451 try again\r\n").expect("BDAT 451");
    let mut unexpected = String::new();
    assert_eq!(reader.read_line(&mut unexpected).expect("no next chunk"), 0);
}

/// Сверяет SMTP command целиком, не только его verb.
fn expect_line(reader: &mut BufReader<TcpStream>, expected: &str) {
    let mut actual = String::new();
    reader.read_line(&mut actual).expect("read SMTP command");
    assert_eq!(actual, expected);
}
