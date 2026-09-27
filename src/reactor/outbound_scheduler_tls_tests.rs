//! Linux integration проверки TLS route постоянного outbound scheduler-а.

use super::{Orchestrator, Session, outbound_scheduler::OutboundScheduler};
use crate::{
    outbound_route::Route,
    platform::linux_poll,
    queue::{OwnedDelivery, State},
    storage_worker::WakingStorageWorker,
    store::Store,
};
use rcgen::{CertificateParams, CertifiedIssuer, KeyPair};
use rustls::{
    ServerConfig, ServerConnection, StreamOwned,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::fd::AsFd,
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[test]
fn scheduler_delivers_through_bound_tls_route() {
    let fixture = Fixture::new("tls-success");
    let (ca, certificate, key) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || serve_tls_message(listener, certificate, key));
    let route = Route::implicit_tls(&address.to_string(), "peer.test", ca).expect("route");
    let pending = run_scheduler(route, &fixture.path, peer);
    assert!(pending.is_empty());
    fixture.remove();
}

#[test]
fn wrong_san_becomes_durable_retry_without_pool_reuse() {
    let fixture = Fixture::new("tls-wrong-san");
    let (ca, certificate, key) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let peer = thread::spawn(move || observe_failed_handshake(listener, certificate, key));
    let route = Route::implicit_tls(&address.to_string(), "wrong.test", ca).expect("route");
    let pending = run_scheduler(route, &fixture.path, peer);
    let Some(delivery) = pending.first() else {
        panic!("failed TLS delivery must remain durable");
    };
    assert_eq!(delivery.state, State::Deferred);
    assert_eq!(delivery.attempts, 1);
    assert!(delivery.next_attempt_epoch > 0);
    fixture.remove();
}

fn run_scheduler(route: Route, path: &std::path::Path, peer: JoinHandle<()>) -> Vec<OwnedDelivery> {
    let mut storage = WakingStorageWorker::start(path, 8).expect("storage");
    let mut lifecycle: Orchestrator<Session> = Orchestrator::new();
    let mut scheduler =
        OutboundScheduler::start(route, &storage, &mut lifecycle).expect("scheduler");
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
                (0, true) => complete_storage(&mut storage, &mut scheduler, &mut lifecycle),
                (1, true) => scheduler
                    .transport_completed(&storage, &mut lifecycle)
                    .expect("transport completion"),
                (_, false) => {}
                _ => panic!("unknown poll target"),
            }
        }
        scheduler.wake_due(&storage, &mut lifecycle).expect("timer");
        if scheduler.deadline().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "TLS scheduler timed out");
    }
    assert!(lifecycle.request_cancel(scheduler.task));
    assert!(
        scheduler
            .wake_due(&storage, &mut lifecycle)
            .expect("cancel")
    );
    scheduler.shutdown(&mut lifecycle).expect("shutdown");
    peer.join().expect("peer");
    storage.shutdown();
    let store = Store::open(path).expect("reopen");
    store.pending_outbound().expect("pending")
}

fn complete_storage(
    storage: &mut WakingStorageWorker,
    scheduler: &mut OutboundScheduler,
    lifecycle: &mut Orchestrator<Session>,
) {
    storage.drain_wakeup().expect("storage wake");
    while let Ok(response) = storage.worker.try_response() {
        scheduler
            .storage_completed(storage, response, lifecycle)
            .expect("storage completion");
    }
}

struct Fixture {
    path: std::path::PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("litterkeep-{label}-{nonce}"));
        let mut store = Store::open(&path).expect("store");
        store
            .append_many_with_outbound(
                &Vec::new(),
                b"Subject: scheduler TLS\r\n\r\nbody\r\n",
                &[("alice@test".into(), "remote@peer.test".into())],
            )
            .expect("queued");
        drop(store);
        Self { path }
    }

    fn remove(self) {
        std::fs::remove_dir_all(self.path).expect("cleanup");
    }
}

fn certificates() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let ca_key = KeyPair::generate().expect("CA key");
    let ca = CertifiedIssuer::self_signed(CertificateParams::default(), ca_key).expect("CA");
    let params = CertificateParams::new(vec!["peer.test".into()]).expect("name");
    let key = KeyPair::generate().expect("server key");
    let certificate = params.signed_by(&key, &ca).expect("certificate");
    (
        ca.der().to_vec(),
        certificate.der().to_vec(),
        key.serialize_der(),
    )
}

fn tls_stream(
    listener: TcpListener,
    certificate: Vec<u8>,
    key: Vec<u8>,
) -> BufReader<StreamOwned<ServerConnection, std::net::TcpStream>> {
    let (stream, _) = listener.accept().expect("accept");
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(certificate)],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key)),
        )
        .expect("server config");
    let connection = ServerConnection::new(Arc::new(config)).expect("connection");
    BufReader::new(StreamOwned::new(connection, stream))
}

fn serve_tls_message(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let mut stream = tls_stream(listener, certificate, key);
    reply(&mut stream, b"220 peer\r\n");
    command(&mut stream, "EHLO");
    reply(&mut stream, b"250 peer\r\n");
    command(&mut stream, "MAIL FROM:");
    reply(&mut stream, b"250 sender\r\n");
    command(&mut stream, "RCPT TO:");
    reply(&mut stream, b"250 recipient\r\n");
    command(&mut stream, "DATA");
    reply(&mut stream, b"354 body\r\n");
    command(&mut stream, "Subject:");
    let mut line = String::new();
    loop {
        line.clear();
        stream.read_line(&mut line).expect("body");
        if line == ".\r\n" {
            break;
        }
    }
    reply(&mut stream, b"250 accepted\r\n");
    command(&mut stream, "QUIT");
    reply(&mut stream, b"221 closing\r\n");
}

fn observe_failed_handshake(listener: TcpListener, certificate: Vec<u8>, key: Vec<u8>) {
    let mut stream = tls_stream(listener, certificate, key);
    let mut byte = [0_u8; 1];
    let _ = stream.read(&mut byte);
}

fn command(
    stream: &mut BufReader<StreamOwned<ServerConnection, std::net::TcpStream>>,
    expected: &str,
) {
    let mut line = String::new();
    stream.read_line(&mut line).expect("command");
    assert!(line.starts_with(expected), "unexpected command: {line}");
}

fn reply(stream: &mut BufReader<StreamOwned<ServerConnection, std::net::TcpStream>>, value: &[u8]) {
    stream.get_mut().write_all(value).expect("reply");
    stream.get_mut().flush().expect("flush");
}

fn readable() -> linux_poll::Interest {
    linux_poll::Interest {
        readable: true,
        writable: false,
    }
}
