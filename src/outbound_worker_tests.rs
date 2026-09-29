//! Проверка фактического владения pool одним bounded outbound worker-ом.

use crate::{
    outbound::DeliveryOutcome,
    outbound_worker::{Job, OutboundWorker},
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

#[test]
fn worker_owns_one_pool_across_two_jobs() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let (ready_sender, ready) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("one accept");
        ready_sender.send(()).expect("ready");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        line(&mut reader, "EHLO");
        stream.write_all(b"250 peer\r\n").expect("ehlo");
        transaction(&mut stream, &mut reader);
        transaction(&mut stream, &mut reader);
        line(&mut reader, "QUIT");
        stream.write_all(b"221 closing\r\n").expect("quit");
    });
    let worker = OutboundWorker::start(address.to_string(), 2, 1).expect("worker");
    worker.submit(job(1, b"first")).expect("first job");
    worker.submit(job(2, b"second")).expect("second job");
    ready.recv().expect("one connection");
    assert_eq!(
        completion(&worker).result.expect("first"),
        DeliveryOutcome::Delivered
    );
    assert_eq!(
        completion(&worker).result.expect("second"),
        DeliveryOutcome::Delivered
    );
    worker.shutdown().expect("graceful SMTP shutdown");
    peer.join().expect("peer");
}

#[test]
fn worker_denies_unrouted_recipient_before_opening_socket() {
    let worker = OutboundWorker::start("127.0.0.1:9", 1, 1).expect("worker");
    worker
        .submit(Job {
            id: 7,
            sender: "alice@litterkeep.test".into(),
            recipient: "alice@litterkeep.test".into(),
            payload: b"body".to_vec(),
            dsn_metadata: None,
        })
        .expect("submit");
    let error = completion(&worker).result.expect_err("route denied");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    worker.shutdown().expect("empty pool shutdown");
}

fn job(id: u64, payload: &[u8]) -> Job {
    Job {
        id,
        sender: "alice@test".into(),
        recipient: "bob@peer.test".into(),
        payload: payload.to_vec(),
        dsn_metadata: None,
    }
}

fn completion(worker: &OutboundWorker) -> crate::outbound_worker::Completion {
    loop {
        match worker.try_completion() {
            Ok(value) => return value,
            Err(std::sync::mpsc::TryRecvError::Empty) => thread::yield_now(),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => panic!("worker disconnected"),
        }
    }
}

fn transaction(stream: &mut std::net::TcpStream, reader: &mut BufReader<std::net::TcpStream>) {
    line(reader, "MAIL FROM:");
    stream.write_all(b"250 sender\r\n").expect("mail");
    line(reader, "RCPT TO:");
    stream.write_all(b"250 recipient\r\n").expect("recipient");
    line(reader, "DATA");
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

fn line(reader: &mut BufReader<std::net::TcpStream>, prefix: &str) {
    let mut value = String::new();
    reader.read_line(&mut value).expect("command");
    assert!(value.starts_with(prefix), "unexpected command: {value}");
}

#[cfg(target_os = "linux")]
#[test]
fn completion_wakes_linux_reactor_socket() {
    use crate::{outbound_wakeup::WakingOutboundWorker, platform::linux_poll};
    use std::os::fd::AsFd;

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream.write_all(b"220 peer ready\r\n").expect("greeting");
        let mut reader = BufReader::new(stream.try_clone().expect("clone"));
        line(&mut reader, "EHLO");
        stream.write_all(b"250 peer\r\n").expect("ehlo");
        transaction(&mut stream, &mut reader);
        line(&mut reader, "QUIT");
        stream.write_all(b"221 closing\r\n").expect("quit");
    });
    let mut worker = WakingOutboundWorker::start(address.to_string(), 1, 1).expect("worker");
    worker.worker.submit(job(1, b"wake")).expect("job");
    let ready = linux_poll::wait(
        &[(
            (worker.wake_reader().as_fd()),
            linux_poll::Interest {
                readable: true,
                writable: false,
            },
        )],
        1_000,
    )
    .expect("poll");
    assert!(ready.first().is_some_and(|entry| entry.readable));
    worker.drain_wakeup().expect("drain wake");
    assert_eq!(
        completion(&worker.worker).result.expect("delivery"),
        DeliveryOutcome::Delivered
    );
    worker.shutdown().expect("graceful SMTP shutdown");
    peer.join().expect("peer");
}
