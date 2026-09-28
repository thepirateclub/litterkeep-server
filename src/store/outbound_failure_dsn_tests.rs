//! Проверка полного отказа внешней доставки и исходящего failure DSN.

use super::*;
use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn external_failure_dsn_is_committed_and_sent_to_original_sender() {
    let path = test_path();
    let request = failure_envelope();
    let source = b"Subject: external delivery\r\n\r\nprivate source body\r\n";
    let mut store = Store::open(&path).expect("open Store");
    store
        .append_many_with_sieve(
            &[],
            source,
            &outbound_recipient(),
            &[],
            Some(&request),
            &[vec![0]],
        )
        .expect("commit original outbound message");
    drop(store);

    let failed_listener = TcpListener::bind("127.0.0.1:0").expect("failure listener");
    let failed_address = failed_listener.local_addr().expect("failure address");
    let failed_peer = thread::spawn(move || reject_recipient(failed_listener));
    let first = crate::outbound_service::drain(&path, &failed_address.to_string(), 1)
        .expect("deliver original message");
    assert_eq!(first.permanent_failures, 1, "{first:?}");
    failed_peer.join().expect("failure peer");

    let reopened = Store::open(&path).expect("reopen after failure");
    let pending = reopened.pending_outbound().expect("pending jobs");
    assert_eq!(
        pending.len(),
        1,
        "failure DSN must be durable outbound work"
    );
    assert_eq!(pending.first().map(|job| job.sender.as_str()), Some(""));
    assert_eq!(
        pending.first().map(|job| job.recipient.as_str()),
        Some("sender@outside.example")
    );
    drop(reopened);

    let report_listener = TcpListener::bind("127.0.0.1:0").expect("report listener");
    let report_address = report_listener.local_addr().expect("report address");
    let report_peer = thread::spawn(move || accept_failure_report(report_listener));
    let second = crate::outbound_service::drain(&path, &report_address.to_string(), 1)
        .expect("send failure DSN");
    assert_eq!(second.delivered, 1, "{second:?}");
    report_peer.join().expect("report peer");

    let completed = Store::open(&path).expect("reopen after report");
    assert!(completed.pending_outbound().expect("queue").is_empty());
    drop(completed);
    std::fs::remove_dir_all(path).expect("remove unique test data");
}

/// Создаёт отдельный тестовый каталог, не пересекаясь с параллельным запуском.
fn test_path() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "litterkeep-failure-dsn-{}-{nonce}",
        std::process::id()
    ))
}

fn outbound_recipient() -> Vec<(String, String)> {
    vec![("alice@litterkeep.test".into(), "remote@peer.test".into())]
}

fn failure_envelope() -> crate::smtp::DsnEnvelope {
    crate::smtp::DsnEnvelope {
        reverse_path: "sender@outside.example".into(),
        mail: crate::smtp::MailDsn::default(),
        recipients: vec![crate::smtp::RcptTo {
            address: "remote@peer.test".into(),
            dsn: crate::smtp::RecipientDsn {
                notify: Some(crate::smtp::NotifyPolicy::Conditions(
                    [crate::smtp::NotifyCondition::Failure]
                        .into_iter()
                        .collect(),
                )),
                original_recipient: None,
            },
        }],
    }
}

/// Отказывает получателю постоянным SMTP 550, сохраняя соединение синхронным.
fn reject_recipient(listener: TcpListener) {
    let (mut stream, _) = listener.accept().expect("accept original SMTP");
    let mut reader = protocol_start(&mut stream);
    expected_command(&mut reader, "MAIL FROM:<alice@litterkeep.test>");
    write_reply(&mut stream, b"250 sender\r\n");
    expected_command(&mut reader, "RCPT TO:<remote@peer.test>");
    write_reply(&mut stream, b"550 5.1.1 no such recipient\r\n");
    expected_command(&mut reader, "RSET");
    write_reply(&mut stream, b"250 reset\r\n");
    expected_command(&mut reader, "QUIT");
    write_reply(&mut stream, b"221 closing\r\n");
}

/// Проверяет внешний envelope и семантику multipart/report независимо от сервера.
fn accept_failure_report(listener: TcpListener) {
    let (mut stream, _) = listener.accept().expect("accept DSN SMTP");
    let mut reader = protocol_start(&mut stream);
    expected_command(&mut reader, "MAIL FROM:<>");
    write_reply(&mut stream, b"250 null sender\r\n");
    expected_command(&mut reader, "RCPT TO:<sender@outside.example>");
    write_reply(&mut stream, b"250 original sender\r\n");
    expected_command(&mut reader, "DATA");
    write_reply(&mut stream, b"354 report data\r\n");
    let body = read_data(&mut reader);
    for marker in [
        b"multipart/report".as_slice(),
        b"Final-Recipient: rfc822; remote@peer.test".as_slice(),
        b"Action: failed".as_slice(),
        b"Status: 5.1.1".as_slice(),
        b"Subject: external delivery".as_slice(),
    ] {
        assert!(
            body.windows(marker.len()).any(|part| part == marker),
            "missing {:?}",
            String::from_utf8_lossy(marker)
        );
    }
    assert!(
        !body
            .windows(b"private source body".len())
            .any(|part| part == b"private source body")
    );
    write_reply(&mut stream, b"250 report accepted\r\n");
    expected_command(&mut reader, "QUIT");
    write_reply(&mut stream, b"221 closing\r\n");
}

fn protocol_start(stream: &mut TcpStream) -> BufReader<TcpStream> {
    write_reply(stream, b"220 independent peer\r\n");
    let mut reader = BufReader::new(stream.try_clone().expect("clone socket"));
    expected_command(&mut reader, "EHLO ");
    write_reply(stream, b"250 peer\r\n");
    reader
}

fn expected_command(reader: &mut BufReader<TcpStream>, expected: &str) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read SMTP command");
    assert!(
        line.starts_with(expected),
        "expected {expected:?}, got {line:?}"
    );
}

fn write_reply(stream: &mut TcpStream, reply: &[u8]) {
    stream.write_all(reply).expect("write SMTP response");
}

fn read_data(reader: &mut BufReader<TcpStream>) -> Vec<u8> {
    let mut body = Vec::new();
    loop {
        let mut line = Vec::new();
        reader.read_until(b'\n', &mut line).expect("read DSN line");
        match line.as_slice() {
            b".\r\n" => break,
            _ => body.extend(line),
        }
    }
    body
}
