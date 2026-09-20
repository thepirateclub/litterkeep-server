//! Linux integration test постоянного scheduler-а с настоящим SMTP peer.

use super::{Orchestrator, Session, outbound_scheduler::OutboundScheduler};
use crate::{platform::linux_poll, storage_worker::WakingStorageWorker, store::Store};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::fd::AsFd;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn scheduler_drains_without_explicit_queue_command() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("litterkeep-live-scheduler-{nonce}"));
    let mut store = Store::open(&path).expect("store");
    store
        .append_many_with_outbound(
            &Vec::new(),
            b"Subject: automatic\r\n\r\nbody\r\n",
            &[("alice@test".into(), "remote@peer.test".into())],
        )
        .expect("queued");
    drop(store);

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_one(listener));
    let mut storage = WakingStorageWorker::start(&path, 8).expect("storage");
    let mut lifecycle: Orchestrator<Session> = Orchestrator::new();
    let mut scheduler = OutboundScheduler::start(&address.to_string(), &storage, &mut lifecycle)
        .expect("scheduler");
    assert_eq!(lifecycle.stats().waiting_tasks, 1);
    assert_eq!(lifecycle.stats().wait_registrations, 1);
    let deadline = Instant::now() + Duration::from_secs(5);

    loop {
        let ready = linux_poll::wait(
            &[
                (storage.wake_reader().as_fd(), readable()),
                (scheduler.wake_reader().as_fd(), readable()),
            ],
            100,
        )
        .expect("poll");
        for (position, item) in ready.into_iter().enumerate() {
            match (position, item.readable) {
                (0, true) => {
                    storage.drain_wakeup().expect("storage wake");
                    while let Ok(response) = storage.worker.try_response() {
                        scheduler
                            .storage_completed(&storage, response, &mut lifecycle)
                            .expect("storage completion");
                    }
                }
                (1, true) => scheduler
                    .transport_completed(&storage, &mut lifecycle)
                    .expect("transport completion"),
                (_, false) => {}
                _ => panic!("unknown poll target"),
            }
        }
        scheduler
            .wake_due(&storage, &mut lifecycle)
            .expect("deadline");
        if let (true, true) = (scheduler.deadline().is_some(), peer.is_finished()) {
            break;
        }
        assert!(Instant::now() < deadline, "scheduler timed out");
    }

    peer.join().expect("peer");
    assert_eq!(lifecycle.stats().waiting_tasks, 1);
    assert_eq!(lifecycle.stats().ready_queue_depth, 0);
    drop(scheduler);
    storage.shutdown();
    let store = Store::open(&path).expect("reopen");
    assert!(store.pending_outbound().expect("pending").is_empty());
    drop(store);
    std::fs::remove_dir_all(path).expect("cleanup");
}

fn readable() -> linux_poll::Interest {
    linux_poll::Interest {
        readable: true,
        writable: false,
    }
}

fn serve_one(listener: TcpListener) {
    let (mut stream, _) = listener.accept().expect("accept");
    stream.write_all(b"220 peer\r\n").expect("greeting");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    exchange(&mut reader, &mut stream, "EHLO", b"250 peer\r\n");
    exchange(&mut reader, &mut stream, "MAIL FROM:", b"250 sender\r\n");
    exchange(&mut reader, &mut stream, "RCPT TO:", b"250 recipient\r\n");
    exchange(&mut reader, &mut stream, "DATA", b"354 body\r\n");
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line).expect("body");
        if line == ".\r\n" {
            break;
        }
    }
    stream.write_all(b"250 accepted\r\n").expect("accepted");
}

fn exchange(
    reader: &mut BufReader<std::net::TcpStream>,
    stream: &mut std::net::TcpStream,
    expected: &str,
    response: &[u8],
) {
    let mut line = String::new();
    reader.read_line(&mut line).expect("command");
    assert!(line.starts_with(expected), "unexpected command: {line}");
    stream.write_all(response).expect("response");
}
