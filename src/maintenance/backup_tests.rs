//! Проверка восстановления UID и точных байтов из согласованной копии.

use super::backup;
use crate::store::Store;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn directory(case: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-backup-{case}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn restored_store_preserves_uid_and_exact_message_bytes() {
    let root = directory("source");
    let copy = directory("copy");
    let raw = b"Subject: backup\r\n\r\nexact bytes";
    let mut store = Store::open(&root).expect("store");
    let uid = store.append("bob", raw).expect("append");
    drop(store);

    let report = backup(&root, &copy).expect("backup");
    assert!(report.files > 0);
    let restored = Store::open(&copy).expect("restored store");
    let messages = restored.messages("bob").expect("messages");
    let message = messages.first().expect("restored message");
    assert_eq!(message.uid, uid);
    assert_eq!(message.bytes, raw);
    drop(restored);
    fs::remove_dir_all(root).expect("source cleanup");
    fs::remove_dir_all(copy).expect("copy cleanup");
}

#[test]
fn rejects_destination_inside_source_without_creating_it() {
    let root = directory("nested");
    let store = Store::open(&root).expect("store");
    drop(store);
    let copy = root.join("backup");

    let error = backup(&root, &copy).expect_err("nested destination");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!copy.exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_a_second_writer_and_existing_destination() {
    let root = directory("lock");
    let copy = directory("existing");
    let store = Store::open(&root).expect("store");
    let locked = backup(&root, &copy).expect_err("exclusive source");
    assert_eq!(locked.kind(), std::io::ErrorKind::AlreadyExists);
    drop(store);
    fs::create_dir(&copy).expect("existing output");
    let existing = backup(&root, &copy).expect_err("existing destination");
    assert_eq!(existing.kind(), std::io::ErrorKind::AlreadyExists);
    fs::remove_dir_all(root).expect("root cleanup");
    fs::remove_dir_all(copy).expect("copy cleanup");
}
