//! Acceptance offline checker-а на достижимых, orphan и отсутствующих blobs.

use super::check;
use crate::store::Store;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn directory(case: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-check-{case}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn reports_reachable_and_orphan_blobs_without_deleting_them() {
    let root = directory("orphan");
    let mut store = Store::open(&root).expect("store");
    store.append("bob", b"reachable").expect("append");
    drop(store);
    let orphan = root.join("blobs/message-999.eml");
    fs::write(&orphan, b"orphan").expect("orphan");

    let report = check(&root).expect("check");
    assert_eq!(report.mailboxes, 1);
    assert_eq!(report.message_links, 1);
    assert_eq!(report.referenced_blobs, 1);
    assert_eq!(report.orphan_blobs, 1);
    assert!(orphan.exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rejects_a_missing_referenced_blob() {
    let root = directory("missing");
    let mut store = Store::open(&root).expect("store");
    let message = store.append("bob", b"lost").expect("append");
    assert_eq!(message, 1);
    drop(store);
    fs::remove_file(root.join("blobs/message-1.eml")).expect("remove blob");

    let error = check(&root).expect_err("missing blob");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn treats_a_pending_delivery_as_a_blob_owner() {
    let root = directory("pending");
    let mut store = Store::open(&root).expect("store");
    store
        .append_many_with_outbound(
            &[],
            b"queued",
            &[(
                "alice@example.test".to_owned(),
                "bob@example.test".to_owned(),
            )],
        )
        .expect("append outbound");
    drop(store);

    let report = check(&root).expect("check");
    assert_eq!(report.message_links, 0);
    assert_eq!(report.referenced_blobs, 1);
    assert_eq!(report.orphan_blobs, 0);
    assert_eq!(report.pending_deliveries, 1);
    fs::remove_dir_all(root).expect("cleanup");
}
