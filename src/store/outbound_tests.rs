//! Проверки Store-to-scheduler bounded outbound handoff.

use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "linux")]
#[path = "outbound_relay_dsn_tests.rs"]
mod relay_delivery;
#[cfg(target_os = "linux")]
#[path = "outbound_service_tests.rs"]
mod service_delivery;

static RUN: AtomicU64 = AtomicU64::new(0);

/// Создаёт изолированный Store root для одной queue handoff проверки.
fn directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-store-outbound-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn due_outbound_limit_bounds_scheduler_handoff() {
    let path = directory();
    let mut store = Store::open(&path).expect("store");
    let no_local_delivery = Vec::new();
    store
        .append_many_with_outbound(
            &no_local_delivery,
            b"first",
            &[("alice@test".into(), "first@remote.test".into())],
        )
        .expect("first job");
    store
        .append_many_with_outbound(
            &no_local_delivery,
            b"second",
            &[("alice@test".into(), "second@remote.test".into())],
        )
        .expect("second job");
    let due = store.due_outbound_limited(0, 1).expect("limited due");
    assert_eq!(due.len(), 1);
    assert_eq!(due.first().expect("one job").recipient, "first@remote.test");
    drop(store);
    let _ = fs::remove_dir_all(path);
}

/// Проверяет независимость сгенерированного outbound payload от входящей почты.
#[test]
fn generated_outbound_payload_is_committed_without_mailbox_link() {
    let path = directory();
    let reply = b"From: bob@litterkeep.test\r\nTo: alice@peer.test\r\n\r\nAway.\r\n";
    let mut store = Store::open(&path).expect("store");
    let transaction_id = store
        .enqueue_outbound_payload("", "alice@peer.test", reply)
        .expect("generated response");
    let delivery = store
        .pending_outbound()
        .expect("pending jobs")
        .into_iter()
        .next()
        .expect("one generated response");
    assert_eq!(delivery.transaction_id, transaction_id);
    assert_eq!(delivery.sender, "");
    assert_eq!(store.outbound_blob(&delivery).expect("payload"), reply);
    store
        .transition_outbound(&delivery, crate::queue::State::Deferred)
        .expect("defer generated response");
    assert!(
        store
            .snapshot_folder("bob", "INBOX")
            .expect("inbox snapshot")
            .snapshot
            .messages
            .is_empty()
    );
    drop(store);

    let mut reopened = Store::open(&path).expect("reopen");
    let recovered = reopened
        .pending_outbound()
        .expect("recovered job")
        .into_iter()
        .next()
        .expect("one recovered response");
    assert_eq!(recovered.sender, "");
    assert_eq!(recovered.state, crate::queue::State::Deferred);
    assert_eq!(
        reopened
            .outbound_blob(&recovered)
            .expect("recovered payload"),
        reply
    );
    assert!(
        reopened
            .snapshot_folder("bob", "INBOX")
            .expect("reopened inbox")
            .snapshot
            .messages
            .is_empty()
    );
    drop(reopened);
    let _ = fs::remove_dir_all(path);
}
