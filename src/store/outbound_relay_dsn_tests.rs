//! Проверяет durable relay DSN от исходящей доставки до независимого SMTP peer.

use super::super::Store;
use crate::{
    outbound_service,
    smtp::{DsnEnvelope, MailDsn, NotifyCondition, NotifyPolicy, RcptTo, RecipientDsn},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

/// Принимающий peer без DSN вызывает report job, который затем доставляется.
#[test]
fn non_dsn_downstream_acceptance_creates_deliverable_relay_notice() {
    let path = test_path();
    let envelope = envelope();
    let source = b"Subject: forward\r\n\r\nprivate source body\r\n";
    let mut store = Store::open(&path).expect("open Store");
    store
        .append_many_with_sieve(
            &[],
            source,
            &[(envelope.reverse_path.clone(), "forward@peer.test".into())],
            &[],
            Some(&envelope),
            &[vec![0]],
        )
        .expect("commit forwarding delivery");
    drop(store);

    let listener = TcpListener::bind("127.0.0.1:0").expect("peer listener");
    let address = listener.local_addr().expect("peer address");
    let peer = thread::spawn(move || {
        accept_forwarded_message(&listener);
        accept_relay_notice(&listener);
    });

    let forwarded =
        outbound_service::drain(&path, &address.to_string(), 1).expect("forward original message");
    assert_eq!(forwarded.delivered, 1, "{forwarded:?}");
    let queued_report = Store::open(&path).expect("reopen Store");
    let pending = queued_report.pending_outbound().expect("pending report");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending.first().map(|job| job.sender.as_str()), Some(""));
    assert_eq!(
        pending.first().map(|job| job.recipient.as_str()),
        Some("sender@outside.test")
    );
    drop(queued_report);

    let report =
        outbound_service::drain(&path, &address.to_string(), 1).expect("deliver relay notice");
    assert_eq!(report.delivered, 1, "{report:?}");
    peer.join().expect("independent peer");
    let completed = Store::open(&path).expect("reopen after report");
    assert!(
        completed
            .pending_outbound()
            .expect("empty queue")
            .is_empty()
    );
    drop(completed);
    std::fs::remove_dir_all(path).expect("remove test data");
}

fn envelope() -> DsnEnvelope {
    DsnEnvelope {
        reverse_path: "sender@outside.test".into(),
        mail: MailDsn::default(),
        recipients: vec![RcptTo {
            address: "forward@peer.test".into(),
            dsn: RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Success].into_iter().collect(),
                )),
                original_recipient: None,
            },
        }],
    }
}

fn test_path() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "litterkeep-relayed-dsn-{}-{nonce}",
        std::process::id()
    ))
}

fn accept_forwarded_message(listener: &TcpListener) {
    let (mut stream, mut reader) = session(listener);
    command(&mut reader, "EHLO");
    stream.write_all(b"250 peer.test\r\n").expect("EHLO reply");
    command(&mut reader, "MAIL FROM:<sender@outside.test>");
    stream.write_all(b"250 sender\r\n").expect("MAIL reply");
    command(&mut reader, "RCPT TO:<forward@peer.test>");
    stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
    command(&mut reader, "DATA");
    stream.write_all(b"354 data\r\n").expect("DATA reply");
    read_data(&mut reader);
    stream
        .write_all(b"250 accepted\r\n")
        .expect("accept message");
    command(&mut reader, "QUIT");
    stream.write_all(b"221 closing\r\n").expect("QUIT reply");
}

fn accept_relay_notice(listener: &TcpListener) {
    let (mut stream, mut reader) = session(listener);
    command(&mut reader, "EHLO");
    stream.write_all(b"250 peer.test\r\n").expect("EHLO reply");
    command(&mut reader, "MAIL FROM:<>");
    stream
        .write_all(b"250 null sender\r\n")
        .expect("MAIL reply");
    command(&mut reader, "RCPT TO:<sender@outside.test>");
    stream
        .write_all(b"250 original sender\r\n")
        .expect("RCPT reply");
    command(&mut reader, "DATA");
    stream
        .write_all(b"354 report data\r\n")
        .expect("DATA reply");
    let report = read_data(&mut reader);
    for field in [
        b"multipart/report".as_slice(),
        b"Action: relayed".as_slice(),
        b"Status: 2.0.0".as_slice(),
        b"Subject: forward".as_slice(),
    ] {
        assert!(contains(&report, field), "missing {:?}", field);
    }
    assert!(!contains(&report, b"private source body"));
    stream
        .write_all(b"250 accepted\r\n")
        .expect("accept report");
    command(&mut reader, "QUIT");
    stream.write_all(b"221 closing\r\n").expect("QUIT reply");
}

fn session(listener: &TcpListener) -> (TcpStream, BufReader<TcpStream>) {
    let (mut stream, _) = listener.accept().expect("accept SMTP session");
    stream
        .write_all(b"220 independent peer\r\n")
        .expect("greeting");
    let reader = BufReader::new(stream.try_clone().expect("clone SMTP stream"));
    (stream, reader)
}

fn command(reader: &mut BufReader<TcpStream>, expected: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read SMTP command");
    assert!(
        line.starts_with(expected),
        "expected {expected:?}, got {line:?}"
    );
}

fn read_data(reader: &mut BufReader<TcpStream>) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line).expect("read SMTP data");
        match line.as_slice() {
            b".\r\n" => return body,
            _ => body.extend(line),
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
