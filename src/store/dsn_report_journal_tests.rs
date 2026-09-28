//! Проверки replay и целостности индекса failure DSN.

use super::Store;
use crate::queue::DsnReportKey;
use std::{
    collections::HashSet,
    fs, io,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-dsn-report-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn key() -> DsnReportKey {
    DsnReportKey {
        transaction_id: 7,
        recipient: "bob@example.test".into(),
        original_recipient_indices: vec![2, 4],
        failure: crate::dsn::Failure::PermanentSmtp,
    }
}

#[test]
fn report_is_idempotent_across_store_reopen() {
    let directory = root();
    let mut store = Store::open(&directory).expect("open Store");
    let first = store
        .append_dsn_report_once("alice", b"failure report", &key())
        .expect("first report")
        .expect("local report");
    let repeated = store
        .append_dsn_report_once("alice", b"duplicate", &key())
        .expect("repeated report");
    assert_eq!(repeated, Some(first));
    drop(store);

    let mut reopened = Store::open(&directory).expect("reopen Store");
    let recovered = reopened
        .append_dsn_report_once("alice", b"duplicate", &key())
        .expect("recovered key");
    assert_eq!(recovered, Some(first));
    let messages = reopened.messages("alice").expect("mailbox");
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages.first().map(|message| message.bytes.as_slice()),
        Some(b"failure report".as_slice())
    );
    drop(reopened);
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn uncommitted_report_mapping_does_not_hide_retry() {
    let directory = root();
    let mut store = Store::open(&directory).expect("open Store");
    store.set_failpoint(super::StoreFailpoint::Stage(
        super::AppendStage::DsnReportWritten,
    ));
    assert!(
        store
            .append_dsn_report_once("alice", b"failure report", &key())
            .is_err()
    );
    drop(store);

    let mut reopened = Store::open(&directory).expect("recover incomplete report");
    assert!(reopened.dsn_report_transactions.is_empty());
    let transaction = reopened
        .append_dsn_report_once("alice", b"retry report", &key())
        .expect("retry")
        .expect("committed report");
    let messages = reopened.messages("alice").expect("mailbox");
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages.first().map(|message| message.bytes.as_slice()),
        Some(b"retry report".as_slice())
    );
    assert_eq!(transaction, 1);
    drop(reopened);
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn recovery_trims_incomplete_report_tail_and_keeps_committed_mapping() {
    let directory = root();
    let mut store = Store::open(&directory).expect("open Store");
    let transaction = store
        .append_dsn_report_once("alice", b"failure report", &key())
        .expect("append")
        .expect("local report");
    drop(store);
    let journal = directory.join("journal/dsn-reports.log");
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .expect("journal")
        .write_all(b"truncated frame")
        .expect("tail");

    let mut reopened = Store::open(&directory).expect("recover journal");
    let retry = reopened
        .append_dsn_report_once("alice", b"duplicate", &key())
        .expect("deduplicated retry");
    assert_eq!(retry, Some(transaction));
    drop(reopened);
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn committed_duplicate_key_with_different_id_is_corruption() {
    let directory = root();
    let mut store = Store::open(&directory).expect("open Store");
    store
        .append_dsn_report_once("alice", b"failure report", &key())
        .expect("append");
    drop(store);

    let mut reopened = Store::open(&directory).expect("reopen Store");
    reopened
        .append("alice", b"second report")
        .expect("second transaction");
    drop(reopened);
    let journal = directory.join("journal/dsn-reports.log");
    let source = fs::read_to_string(&journal).expect("read journal");
    let first = source.lines().next().expect("first frame");
    let fields = first.split_whitespace().collect::<Vec<_>>();
    let payload = format!(
        "2 {} {} {} {}",
        fields.get(1).expect("source transaction"),
        fields.get(2).expect("ordinal"),
        fields.get(3).expect("failure"),
        fields.get(4).expect("recipient")
    );
    let checksum = payload
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |mut value, byte| {
            value ^= u64::from(*byte);
            value.wrapping_mul(0x100_0000_01b3)
        });
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .expect("journal")
        .write_all(format!("{payload} {checksum:016x}\n").as_bytes())
        .expect("duplicate");

    let error = Store::open(&directory).expect_err("duplicate IDs conflict");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn journal_loader_ignores_uncommitted_transaction_ids() {
    let directory = root();
    fs::create_dir_all(directory.join("journal")).expect("journal dir");
    super::dsn_report_journal::append(&directory, 4, &key()).expect("frame");
    let loaded = super::dsn_report_journal::load(&directory, &HashSet::new()).expect("load");
    assert!(loaded.is_empty());
    fs::remove_dir_all(directory).expect("cleanup");
}
