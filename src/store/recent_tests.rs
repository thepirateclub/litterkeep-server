//! Проверки lifetime `\\Recent`: delivery, assignment и recovery.

use super::*;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-recent-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn select_claims_recent_once_and_claim_survives_restart() {
    let root = root();
    let mut store = Store::open(&root).expect("store");
    let key = store.resolve_folder("bob", "INBOX").expect("inbox");
    store.append(&key, b"first").expect("append");
    drop(store);

    let mut reopened = Store::open(&root).expect("reopen");
    assert_eq!(
        reopened
            .snapshot_folder("bob", "INBOX")
            .expect("status snapshot")
            .recent_uids,
        vec![1]
    );
    let selected = reopened.select_folder("bob", "INBOX").expect("select");
    assert_eq!(selected.recent_uids, vec![1]);
    assert!(
        reopened
            .snapshot_folder("bob", "INBOX")
            .expect("claimed")
            .recent_uids
            .is_empty()
    );
    drop(reopened);

    let mut final_store = Store::open(&root).expect("final reopen");
    assert!(
        final_store
            .snapshot_folder("bob", "INBOX")
            .expect("no replay")
            .recent_uids
            .is_empty()
    );
    drop(final_store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn copy_and_expunge_keep_recent_projection_consistent() {
    let root = root();
    let mut store = Store::open(&root).expect("store");
    store.append("source", b"message").expect("append");
    store.copy("source", "destination", &[1]).expect("copy");
    assert_eq!(store.recent.get("destination").map(BTreeSet::len), Some(1));
    store.expunge("source", &[1]).expect("expunge");
    assert!(store.recent.get("source").is_none_or(BTreeSet::is_empty));
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn incomplete_recent_tail_is_trimmed_without_losing_committed_delivery() {
    let root = root();
    let mut store = Store::open(&root).expect("store");
    store.append("bob", b"message").expect("append");
    drop(store);
    let journal = root.join("journal/recent.log");
    let before = fs::metadata(&journal).expect("metadata").len();
    OpenOptions::new()
        .append(true)
        .open(&journal)
        .expect("open journal")
        .write_all(b"LKRC\x00")
        .expect("partial tail");
    let reopened = Store::open(&root).expect("reopen");
    assert_eq!(reopened.recent.get("bob").map(BTreeSet::len), Some(1));
    assert_eq!(fs::metadata(&journal).expect("trimmed").len(), before);
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}
