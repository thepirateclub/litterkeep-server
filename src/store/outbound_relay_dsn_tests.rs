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

/// Два временных отказа сохраняют и публикуют delay DSN до следующего retry.
#[test]
fn repeated_temporary_failures_publish_delayed_report_and_keep_retry() {
    let path = test_path();
    let mut envelope = envelope();
    let recipient = envelope
        .recipients
        .first_mut()
        .expect("one forwarding recipient");
    recipient.dsn.notify = Some(NotifyPolicy::Conditions(
        [NotifyCondition::Delay].into_iter().collect(),
    ));
    let source = b"Subject: delayed\r\n\r\nprivate source body\r\n";
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
        reject_temporary_mail(&listener);
        reject_temporary_mail(&listener);
        accept_delayed_notice(&listener);
    });

    let first =
        outbound_service::drain(&path, &address.to_string(), 1).expect("first temporary attempt");
    assert_eq!(first.deferred, 1);
    make_deferred_job_due(&path);
    let second =
        outbound_service::drain(&path, &address.to_string(), 2).expect("second temporary attempt");
    assert_eq!(second.deferred, 1);
    assert_eq!(second.delivered, 1, "{second:?}");
    let queued = Store::open(&path).expect("reopen delayed queue");
    let jobs = queued.pending_outbound().expect("pending jobs");
    assert!(
        jobs.iter()
            .any(|job| job.state == crate::queue::State::Deferred)
    );
    assert!(!jobs.iter().any(|job| job.sender.is_empty()));
    let original = jobs
        .iter()
        .find(|job| !job.sender.is_empty())
        .expect("retryable original job");
    assert_eq!(original.attempts, 2);
    drop(queued);

    peer.join().expect("independent SMTP peers");
    std::fs::remove_dir_all(path).expect("remove test data");
}

/// Делает сохранённую retry запись due без wall-clock ожидания в тесте.
fn make_deferred_job_due(path: &std::path::Path) {
    let mut store = Store::open(path).expect("open retry Store");
    let mut delivery = store
        .pending_outbound()
        .expect("pending retry")
        .into_iter()
        .next()
        .expect("one original route");
    delivery.next_attempt_epoch = 0;
    store
        .transition_outbound(&delivery, crate::queue::State::Deferred)
        .expect("persist test deadline");
}

/// Отказывает на MAIL временным 421, чтобы transport немедленно закрыл канал.
fn reject_temporary_mail(listener: &TcpListener) {
    let (mut stream, mut reader) = session(listener);
    command(&mut reader, "EHLO");
    stream.write_all(b"250 peer.test\r\n").expect("EHLO reply");
    command(&mut reader, "MAIL FROM:");
    stream
        .write_all(b"421 4.3.2 temporary peer shutdown\r\n")
        .expect("temporary MAIL reply");
}

/// Проверяет отправку delay DSN с null reverse-path независимому peer-у.
fn accept_delayed_notice(listener: &TcpListener) {
    let (mut stream, mut reader) = session(listener);
    command(&mut reader, "EHLO");
    stream.write_all(b"250 peer.test\r\n").expect("EHLO reply");
    command(&mut reader, "MAIL FROM:<>");
    stream.write_all(b"250 sender\r\n").expect("MAIL reply");
    command(&mut reader, "RCPT TO:<sender@outside.test>");
    stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
    command(&mut reader, "DATA");
    stream.write_all(b"354 data\r\n").expect("DATA reply");
    let report = read_data(&mut reader);
    assert!(contains(&report, b"Action: delayed"));
    assert!(contains(&report, b"Status: 4.0.0"));
    stream
        .write_all(b"250 accepted\r\n")
        .expect("accept report");
    command(&mut reader, "QUIT");
    stream.write_all(b"221 closing\r\n").expect("QUIT reply");
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
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("bounded test peer wait");
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
