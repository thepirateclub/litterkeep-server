//! Проверяет reuse реального TCP channel, а не только внутренний счётчик pool.
use crate::{outbound::DeliveryOutcome, outbound_pool::ConnectionPool};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

#[test]
fn clean_transactions_reuse_one_peer_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let (accepted_sender, accepted) = mpsc::channel();
    let (observed_sender, observed) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        accepted_sender.send(()).expect("accepted signal");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        read_line(&mut reader, "EHLO");
        stream.write_all(b"250 peer\r\n").expect("ehlo");
        transaction(&mut stream, &mut reader);
        transaction(&mut stream, &mut reader);
        observed_sender.send(()).expect("observed signal");
    });
    let mut pool = ConnectionPool::new(address.to_string(), 1).expect("pool");
    assert_eq!(
        pool.deliver("alice@test", "one@test", b"one")
            .expect("first"),
        DeliveryOutcome::Delivered
    );
    assert_eq!(
        pool.deliver("alice@test", "two@test", b"two")
            .expect("second"),
        DeliveryOutcome::Delivered
    );
    accepted.recv().expect("one accepted connection");
    observed.recv().expect("both transactions");
    assert_eq!(pool.open_count(), 1);
    assert_eq!(pool.idle_count(), 1);
    peer.join().expect("peer");
}

#[test]
fn rejected_transaction_is_dropped_before_next_recipient() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        let (mut rejected, _) = listener.accept().expect("first accept");
        greeting_and_ehlo(&mut rejected);
        let mut rejected_reader = BufReader::new(rejected.try_clone().expect("first clone"));
        read_line(&mut rejected_reader, "MAIL FROM:");
        rejected.write_all(b"250 sender\r\n").expect("sender reply");
        read_line(&mut rejected_reader, "RCPT TO:");
        rejected.write_all(b"450 busy\r\n").expect("deferred reply");
        let (mut clean, _) = listener.accept().expect("second accept");
        greeting_and_ehlo(&mut clean);
        let mut clean_reader = BufReader::new(clean.try_clone().expect("second clone"));
        transaction(&mut clean, &mut clean_reader);
    });
    let mut pool = ConnectionPool::new(address.to_string(), 1).expect("pool");
    assert_eq!(
        pool.deliver("alice@test", "first@test", b"first")
            .expect("deferred result"),
        DeliveryOutcome::Deferred
    );
    assert_eq!(pool.open_count(), 0);
    assert_eq!(
        pool.deliver("alice@test", "second@test", b"second")
            .expect("second result"),
        DeliveryOutcome::Delivered
    );
    assert_eq!(pool.open_count(), 1);
    assert_eq!(pool.idle_count(), 1);
    peer.join().expect("peer");
}

fn greeting_and_ehlo(stream: &mut std::net::TcpStream) {
    stream.write_all(b"220 peer ready\r\n").expect("greeting");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    read_line(&mut reader, "EHLO");
    stream.write_all(b"250 peer\r\n").expect("ehlo");
}

fn transaction(stream: &mut std::net::TcpStream, reader: &mut BufReader<std::net::TcpStream>) {
    read_line(reader, "MAIL FROM:");
    stream.write_all(b"250 sender\r\n").expect("sender reply");
    read_line(reader, "RCPT TO:");
    stream
        .write_all(b"250 recipient\r\n")
        .expect("recipient reply");
    read_line(reader, "DATA");
    stream.write_all(b"354 send data\r\n").expect("data reply");
    read_data(reader);
    stream
        .write_all(b"250 accepted\r\n")
        .expect("accepted reply");
}

fn read_line(reader: &mut BufReader<std::net::TcpStream>, prefix: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("command");
    assert!(line.starts_with(prefix), "unexpected command: {line}");
}

fn read_data(reader: &mut BufReader<std::net::TcpStream>) {
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("message line");
        if line == ".\r\n" {
            return;
        }
    }
}
