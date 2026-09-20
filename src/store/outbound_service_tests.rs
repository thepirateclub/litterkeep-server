//! Linux end-to-end проверка service-owned durable outbound drain.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

#[test]
fn service_drain_commits_delivery_after_transport_completion() {
    let path = std::env::temp_dir().join("litterkeep-outbound-service-test");
    let _ = fs::remove_dir_all(&path);
    let mut store = Store::open(&path).expect("store");
    store
        .append_many_with_outbound(
            &Vec::new(),
            b"Subject: service\r\n\r\nbody\r\n",
            &[("alice@test".into(), "remote@peer.test".into())],
        )
        .expect("queued");
    drop(store);
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_one(listener));
    let report = crate::outbound_service::drain(&path, &address.to_string(), 1).expect("drain");
    assert_eq!(report.delivered, 1);
    peer.join().expect("peer");
    let store = Store::open(&path).expect("reopen");
    assert!(store.pending_outbound().expect("pending").is_empty());
    drop(store);
    fs::remove_dir_all(path).expect("cleanup");
}

fn serve_one(listener: TcpListener) {
    let (mut stream, _) = listener.accept().expect("accept");
    stream.write_all(b"220 peer\r\n").expect("greeting");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    command(&mut reader, "EHLO");
    stream.write_all(b"250 peer\r\n").expect("ehlo");
    command(&mut reader, "MAIL FROM:");
    stream.write_all(b"250 sender\r\n").expect("sender");
    command(&mut reader, "RCPT TO:");
    stream.write_all(b"250 recipient\r\n").expect("recipient");
    command(&mut reader, "DATA");
    stream.write_all(b"354 body\r\n").expect("data");
    let mut body = String::new();
    loop {
        body.clear();
        reader.read_line(&mut body).expect("body");
        if body == ".\r\n" {
            break;
        }
    }
    stream.write_all(b"250 accepted\r\n").expect("accepted");
}

fn command(reader: &mut BufReader<std::net::TcpStream>, expected: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("command");
    assert!(line.starts_with(expected), "unexpected command: {line}");
}
