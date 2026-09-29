//! Проверяет передачу RFC 3461 policy через реальный outbound SMTP worker.

use crate::{
    outbound::DeliveryOutcome,
    outbound_worker::{Job, OutboundWorker},
    queue::DsnDeliveryMetadata,
    smtp::{
        MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient,
        RecipientDsn, ReturnContent,
    },
};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

#[test]
fn worker_forwards_stored_mail_and_recipient_dsn_values() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let (commands_sender, commands) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        read_line(&mut reader, "EHLO");
        stream
            .write_all(b"250-peer.test\r\n250-DSN\r\n250 SIZE 100000\r\n")
            .expect("EHLO capabilities");
        let mail = read_line(&mut reader, "MAIL FROM:");
        commands_sender.send(mail).expect("MAIL observed");
        stream.write_all(b"250 sender\r\n").expect("MAIL reply");
        let recipient = read_line(&mut reader, "RCPT TO:");
        commands_sender.send(recipient).expect("RCPT observed");
        stream.write_all(b"250 recipient\r\n").expect("RCPT reply");
        read_line(&mut reader, "DATA");
        stream.write_all(b"354 data\r\n").expect("DATA reply");
        read_data(&mut reader);
        stream.write_all(b"250 accepted\r\n").expect("accepted");
        read_line(&mut reader, "QUIT");
        stream.write_all(b"221 closing\r\n").expect("QUIT reply");
    });

    let worker = OutboundWorker::start(address.to_string(), 1, 1).expect("worker");
    worker
        .submit(Job {
            id: 41,
            sender: "alice@origin.test".into(),
            recipient: "bob@forward.test".into(),
            payload: b"Subject: forward\r\n\r\nbody".to_vec(),
            dsn_metadata: Some(metadata()),
        })
        .expect("job");
    assert_eq!(completion(&worker), DeliveryOutcome::Delivered);
    assert_eq!(
        commands.recv().expect("MAIL command"),
        "MAIL FROM:<alice@origin.test> SIZE=26 RET=HDRS ENVID=trace+20id+2B1\r\n"
    );
    assert_eq!(
        commands.recv().expect("RCPT command"),
        "RCPT TO:<bob@forward.test> NOTIFY=FAILURE,SUCCESS ORCPT=rfc822;Alias+20User@example.test\r\n"
    );
    worker.shutdown().expect("worker shutdown");
    peer.join().expect("peer");
}

/// Проверяет локальный `relayed` результат, если accepted peer не имеет DSN.
#[test]
fn worker_reports_relayed_after_acceptance_by_peer_without_dsn() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        read_line(&mut reader, "EHLO");
        stream.write_all(b"250 peer.test\r\n").expect("EHLO");
        let mail = read_line(&mut reader, "MAIL FROM:");
        assert_eq!(
            (mail.contains("RET="), mail.contains("ENVID=")),
            (false, false)
        );
        stream.write_all(b"250 sender\r\n").expect("MAIL");
        let recipient = read_line(&mut reader, "RCPT TO:");
        assert_eq!(
            (recipient.contains("NOTIFY="), recipient.contains("ORCPT=")),
            (false, false)
        );
        stream.write_all(b"250 recipient\r\n").expect("RCPT");
        read_line(&mut reader, "DATA");
        stream.write_all(b"354 data\r\n").expect("DATA");
        read_data(&mut reader);
        stream.write_all(b"250 accepted\r\n").expect("accepted");
        read_line(&mut reader, "QUIT");
        stream.write_all(b"221 closing\r\n").expect("QUIT");
    });

    let worker = OutboundWorker::start(address.to_string(), 1, 1).expect("worker");
    let mut metadata = metadata();
    metadata.mail.return_content = None;
    metadata.mail.envelope_id = None;
    worker
        .submit(Job {
            id: 43,
            sender: "alice@origin.test".into(),
            recipient: "bob@forward.test".into(),
            payload: b"Subject: forward\r\n\r\nbody".to_vec(),
            dsn_metadata: Some(metadata),
        })
        .expect("job");
    assert_eq!(completion(&worker), DeliveryOutcome::Relayed);
    worker.shutdown().expect("worker shutdown");
    peer.join().expect("peer");
}

#[test]
fn worker_defers_multi_origin_alias_without_starting_remote_transaction() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        read_line(&mut reader, "EHLO");
        stream.write_all(b"250 peer.test\r\n").expect("EHLO");
        read_line(&mut reader, "RSET");
        stream.write_all(b"250 reset\r\n").expect("RSET");
        read_line(&mut reader, "QUIT");
        stream.write_all(b"221 closing\r\n").expect("QUIT");
    });

    let worker = OutboundWorker::start(address.to_string(), 1, 1).expect("worker");
    let mut metadata = metadata();
    metadata
        .origin_recipients
        .push(metadata.origin_recipients.first().cloned().flatten());
    worker
        .submit(Job {
            id: 42,
            sender: "alice@origin.test".into(),
            recipient: "bob@forward.test".into(),
            payload: b"Subject: grouped\r\n\r\nbody".to_vec(),
            dsn_metadata: Some(metadata),
        })
        .expect("job");
    assert_eq!(completion(&worker), DeliveryOutcome::Deferred);
    worker.shutdown().expect("worker shutdown");
    peer.join().expect("peer");
}

fn metadata() -> DsnDeliveryMetadata {
    DsnDeliveryMetadata {
        mail: MailDsn {
            return_content: Some(ReturnContent::Headers),
            envelope_id: Some(b"trace id+1".to_vec()),
        },
        recipient: None,
        origin_recipients: vec![Some(RecipientDsn {
            notify: Some(NotifyPolicy::Conditions(HashSet::from([
                NotifyCondition::Failure,
                NotifyCondition::Success,
            ]))),
            original_recipient: Some(OriginalRecipient {
                address_type: OriginalAddressType::Rfc822,
                address: b"Alias User@example.test".to_vec(),
            }),
        })],
    }
}

fn completion(worker: &OutboundWorker) -> DeliveryOutcome {
    loop {
        match worker.try_completion() {
            Ok(completion) => return completion.result.expect("transport result"),
            Err(std::sync::mpsc::TryRecvError::Empty) => thread::yield_now(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("worker disconnected"),
        }
    }
}

fn read_line(reader: &mut BufReader<std::net::TcpStream>, prefix: &str) -> String {
    let mut line = String::new();
    reader.read_line(&mut line).expect("SMTP command");
    assert!(line.starts_with(prefix), "unexpected command: {line}");
    line
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
