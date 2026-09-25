//! Интеграция AUTHENTICATE continuation с настоящим Argon2 account worker-ом.

use super::super::super::*;
use super::*;
use crate::{accounts::Accounts, platform::linux_poll};
use std::os::fd::AsFd;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn continuation_authenticates_through_owner_and_rejects_reauthentication() {
    let root = temporary_root();
    let accounts = Accounts::open(&root).expect("accounts");
    accounts.create("bob", b"bob-password").expect("account");
    drop(accounts);
    let mut worker = WakingStorageWorker::start(&root, 2).expect("worker");
    let (mut session, _peer) = imap_session();
    session.imap_authenticate = Some(State {
        tag: "A1".into(),
        mechanism: imap::AuthenticationMechanism::Plain,
    });
    let mut owners = HashMap::new();
    let mut request_id = 1;

    continuation(
        &mut session,
        b"AGJvYgBib2ItcGFzc3dvcmQ=\r\n",
        0,
        1,
        &mut worker,
        &mut owners,
        &mut request_id,
    )
    .expect("continuation");
    let ready = linux_poll::wait(
        &[(
            worker.wake_reader().as_fd(),
            Interest {
                readable: true,
                writable: false,
            },
        )],
        5_000,
    )
    .expect("poll");
    assert!(ready.first().is_some_and(|event| event.readable));
    worker.drain_wakeup().expect("drain");
    let response = worker.worker.try_response().expect("response");
    let pending = session.pending.take().expect("pending");
    completion::write_completion(&mut session, pending, response).expect("completion");
    assert_eq!(session.authenticated_account.as_deref(), Some("bob"));
    assert!(
        session
            .output
            .ends_with(b"A1 OK AUTHENTICATE completed\r\n")
    );

    session.output.clear();
    begin(
        &mut session,
        "A2".into(),
        imap::AuthenticationMechanism::Plain,
        imap::InitialResponse::Present("AGJvYgBib2ItcGFzc3dvcmQ=".into()),
        0,
        1,
        &mut worker,
        &mut owners,
        &mut request_id,
    )
    .expect("reauthentication response");
    assert_eq!(session.output, b"A2 BAD already authenticated\r\n");
    worker.shutdown();
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn plaintext_is_rejected_and_continuation_can_be_cancelled() {
    let root = temporary_root();
    let mut worker = WakingStorageWorker::start(&root, 1).expect("worker");
    let (mut session, _peer) = imap_session();
    let mut owners = HashMap::new();
    let mut request_id = 1;
    begin(
        &mut session,
        "P1".into(),
        imap::AuthenticationMechanism::Plain,
        imap::InitialResponse::Absent,
        0,
        1,
        &mut worker,
        &mut owners,
        &mut request_id,
    )
    .expect("privacy response");
    assert_eq!(
        session.output,
        b"P1 NO [PRIVACYREQUIRED] STARTTLS required\r\n"
    );

    session.output.clear();
    session.imap_authenticate = Some(State {
        tag: "P2".into(),
        mechanism: imap::AuthenticationMechanism::Plain,
    });
    continuation(
        &mut session,
        b"*\r\n",
        0,
        1,
        &mut worker,
        &mut owners,
        &mut request_id,
    )
    .expect("cancel");
    assert_eq!(session.output, b"P2 BAD AUTHENTICATE cancelled\r\n");
    assert!(session.imap_authenticate.is_none());
    assert!(session.pending.is_none());
    worker.shutdown();
    std::fs::remove_dir_all(root).expect("cleanup");
}

fn imap_session() -> (Session, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let peer = TcpStream::connect(listener.local_addr().expect("address")).expect("peer");
    let (stream, _) = listener.accept().expect("accept");
    let mut session = Session::new(Kind::Imap, stream, 1, None, None).expect("session");
    session.output.clear();
    (session, peer)
}

fn temporary_root() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-imap-auth-{nonce}"))
}
