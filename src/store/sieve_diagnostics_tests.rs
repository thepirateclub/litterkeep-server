//! Recovery-проверки отдельного транзакционного журнала ошибок Sieve.

use super::Store;
use super::sieve_diagnostics::{self, PendingDiagnostic};
use crate::sieve::ExecutionError;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn reopen_trims_incomplete_tail_and_preserves_committed_diagnostic() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    install_runtime_failure(&mut store);
    store
        .deliver_sieve_many(
            &["bob".into()],
            "alice@litterkeep.test",
            &["bob@litterkeep.test".into()],
            b"Subject: diagnostic tail\r\n\r\nbody",
        )
        .expect("deliver");
    drop(store);
    let path = root.join("journal/sieve-errors.log");
    OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(b"1\tbroken-tail"))
        .expect("partial tail");
    let store = Store::open(&root).expect("recover");
    assert_eq!(
        store.sieve_diagnostics("bob").expect("diagnostics").len(),
        1
    );
    assert!(fs::read(path).expect("journal").ends_with(b"\n"));
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn matcher_work_limit_falls_back_and_persists_its_typed_diagnostic() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    let pattern = "*a".repeat(1_100);
    let source = format!("if header :matches \"Subject\" \"{pattern}\" {{ discard; }}");
    store
        .put_sieve_script("bob", "matcher-limit", source)
        .expect("put");
    store
        .activate_sieve_script("bob", Some("matcher-limit"))
        .expect("activate");

    store
        .deliver_sieve_many(
            &["bob".into()],
            "alice@litterkeep.test",
            &["bob@litterkeep.test".into()],
            format!("Subject: {}b\r\n\r\nbody", "a".repeat(1_100)).as_bytes(),
        )
        .expect("runtime limit uses safe fallback");

    assert_eq!(
        store
            .snapshot_folder("bob", "INBOX")
            .expect("fallback mailbox")
            .snapshot
            .messages
            .len(),
        1,
        "runtime error must not apply the partial discard action"
    );
    let diagnostics = store.sieve_diagnostics("bob").expect("diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics.first().map(|item| item.error),
        Some(ExecutionError::MatcherLimit)
    );
    drop(store);
    let mut store = Store::open(&root).expect("recover typed diagnostic");
    assert_eq!(
        store
            .sieve_diagnostics("bob")
            .expect("recovered diagnostics")
            .first()
            .map(|item| item.error),
        Some(ExecutionError::MatcherLimit)
    );
    assert_eq!(
        store
            .snapshot_folder("bob", "INBOX")
            .expect("recovered fallback mailbox")
            .snapshot
            .messages
            .len(),
        1
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reopen_rejects_corrupt_complete_diagnostic_frame() {
    let root = directory();
    let store = Store::open(&root).expect("open");
    drop(store);
    fs::write(root.join("journal/sieve-errors.log"), b"1\tbroken\n").expect("corrupt");
    let error = Store::open(&root).expect_err("must reject corruption");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn diagnostic_journal_rejects_growth_beyond_its_disk_budget() {
    let root = directory();
    let store = Store::open(&root).expect("open");
    drop(store);
    let path = root.join("journal/sieve-errors.log");
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .expect("journal");
    file.set_len(sieve_diagnostics::MAX_JOURNAL_BYTES)
        .expect("fill budget");
    file.sync_all().expect("sync budget");
    let pending = PendingDiagnostic {
        account: "bob".to_owned(),
        script_version: 1,
        error: ExecutionError::CommandLimit,
    };

    let error = sieve_diagnostics::append(&root, 1, &[pending]).expect_err("quota");
    assert_eq!(error.kind(), std::io::ErrorKind::StorageFull);
    fs::remove_dir_all(root).expect("cleanup");
}

fn install_runtime_failure(store: &mut Store) {
    let source = (0..17)
        .map(|number| format!("redirect \"remote-{number}@peer.test\";"))
        .collect::<Vec<_>>()
        .join(" ");
    store
        .put_sieve_script("bob", "failure", source)
        .expect("put");
    store
        .activate_sieve_script("bob", Some("failure"))
        .expect("activate");
}

fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-sieve-diagnostic-{nonce}"))
}
