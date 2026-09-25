//! Сквозная проверка Sieve action, Store suppression и generated queue job.

use super::Store;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn vacation_delivers_original_and_suppresses_duplicate_response_after_reopen() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store);
    store
        .deliver_sieve_many(
            &["bob".into()],
            "alice@peer.test",
            &["bob@litterkeep.test".into()],
            b"To: bob@litterkeep.test\r\nSubject: Original\r\nMessage-ID: <first@peer.test>\r\n\r\nmessage",
        )
        .expect("first incoming message");
    assert_eq!(folder_count(&mut store), 1);
    assert_eq!(outbound_count(&store), 1);
    let reply = pending_reply(&store);
    assert_eq!(reply.sender, "");
    assert_eq!(reply.recipient, "alice@peer.test");
    let bytes = store.outbound_blob(&reply).expect("reply payload");
    assert!(
        bytes
            .windows(b"Auto-Submitted: auto-replied\r\n".len())
            .any(|part| part == b"Auto-Submitted: auto-replied\r\n")
    );
    drop(store);

    let mut reopened = Store::open(&root).expect("reopen");
    reopened
        .deliver_sieve_many(
            &["bob".into()],
            "alice@peer.test",
            &["bob@litterkeep.test".into()],
            b"To: bob@litterkeep.test\r\nSubject: Another message\r\nMessage-ID: <second@peer.test>\r\n\r\nmessage",
        )
        .expect("second incoming message");
    assert_eq!(folder_count(&mut reopened), 2);
    assert_eq!(outbound_count(&reopened), 1);
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn vacation_safety_policy_suppresses_list_and_auto_submitted_messages() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store);
    for header in [
        b"List-Id: <news.peer.test>\r\n".as_slice(),
        b"Auto-Submitted: auto-generated\r\n".as_slice(),
    ] {
        let mut message = b"To: bob@litterkeep.test\r\nSubject: Automated\r\n".to_vec();
        message.extend_from_slice(header);
        message.extend_from_slice(b"\r\nbody");
        store
            .deliver_sieve_many(
                &["bob".into()],
                "alice@peer.test",
                &["bob@litterkeep.test".into()],
                &message,
            )
            .expect("incoming message remains delivered");
    }
    assert_eq!(folder_count(&mut store), 2);
    assert_eq!(outbound_count(&store), 0);
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

fn activate(store: &mut Store) {
    store
        .put_sieve_script(
            "bob",
            "away",
            "require \"vacation\"; vacation :days 1 :subject \"Away\" \"I am away\";".into(),
        )
        .expect("script");
    store
        .activate_sieve_script("bob", Some("away"))
        .expect("active");
}

fn pending_reply(store: &Store) -> crate::queue::OwnedDelivery {
    store
        .pending_outbound()
        .expect("queue")
        .into_iter()
        .next()
        .expect("one reply")
}

fn outbound_count(store: &Store) -> usize {
    store.pending_outbound().expect("queue").len()
}

fn folder_count(store: &mut Store) -> usize {
    store
        .snapshot_folder("bob", "INBOX")
        .expect("INBOX")
        .snapshot
        .messages
        .len()
}

fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-store-vacation-{nonce}"))
}
