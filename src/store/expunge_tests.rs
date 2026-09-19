//! Проверки обычных и transaction-зависимых tombstone хранилища.

use super::Store;
use super::expunges;
use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-expunge-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn expunge_hides_only_deleted_uid_and_survives_restart() {
    let root = root();
    let mut store = Store::open(&root).expect("store");
    store.append("bob", b"first").expect("first");
    store.append("bob", b"second").expect("second");
    store
        .update_flags(
            "bob",
            &[1],
            FlagMutation::Add,
            MessageFlags::from_flags([SystemFlag::Deleted]),
        )
        .expect("deleted flag");
    assert_eq!(store.expunge("bob", &[1]).expect("expunge"), vec![1]);
    drop(store);

    let journal = root.join("mailboxes").join("bob.expunges");
    let committed_length = fs::metadata(&journal).expect("metadata").len();
    let mut file = OpenOptions::new()
        .append(true)
        .open(&journal)
        .expect("journal");
    file.write_all(b"LKEX\0\x01\0\0\0\x01")
        .expect("partial tombstone");
    file.sync_all().expect("sync partial");
    drop(file);

    let reopened = Store::open(&root).expect("reopen");
    assert_eq!(
        fs::metadata(&journal).expect("metadata").len(),
        committed_length
    );
    let snapshot = reopened.snapshot("bob").expect("recovered snapshot");
    assert_eq!(snapshot.messages.len(), 1);
    assert_eq!(
        snapshot.messages.first().map(|message| message.uid),
        Some(2)
    );
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn uncommitted_transaction_tombstone_keeps_source_message() {
    let root = root();
    let mut store = Store::open(&root).expect("store");
    store.append("source", b"source").expect("append");
    let transaction_id = store.next_transaction_id;
    expunges::write_transaction(&root, "source", transaction_id, &[1]).expect("tombstone");
    drop(store);

    let reopened = Store::open(&root).expect("reopen");
    assert_eq!(
        reopened
            .snapshot("source")
            .expect("snapshot")
            .messages
            .len(),
        1
    );
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}
