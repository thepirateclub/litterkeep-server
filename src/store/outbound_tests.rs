//! Проверки Store-to-scheduler bounded outbound handoff.

use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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
