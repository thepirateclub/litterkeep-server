//! Границы ёмкости и recovery RFC 5230 response suppression journal.

use super::vacation_compaction;
use super::vacation_suppression::{self, PendingClaim, Record, VacationKey};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn committed_history_retains_at_least_one_thousand_distinct_responses() {
    let root = directory();
    fs::create_dir_all(root.join("journal")).expect("journal directory");
    let account = "b".repeat(256);
    let sender = "a".repeat(320);
    let suffix = "x".repeat(252);
    let committed = HashSet::from([41]);
    for number in 0..1_000 {
        let claim = PendingClaim {
            key: VacationKey {
                account: account.clone(),
                sender: sender.clone(),
                identity: format!("{number:04}{suffix}"),
            },
            expires_at: 10_000,
            created_at: 100,
        };
        vacation_suppression::append(&root, 41, Some(&claim), &committed).expect("append history");
    }

    let loaded = vacation_suppression::load(&root, &committed).expect("load history");
    assert_eq!(loaded.len(), 1_000);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recovery_trims_incomplete_tail_after_retaining_committed_records() {
    let root = directory();
    fs::create_dir_all(root.join("journal")).expect("journal directory");
    let claim = PendingClaim {
        key: VacationKey {
            account: "bob".into(),
            sender: "alice@peer.test".into(),
            identity: "stable-response".into(),
        },
        expires_at: 10_000,
        created_at: 100,
    };
    let committed = HashSet::from([41]);
    vacation_suppression::append(&root, 41, Some(&claim), &committed).expect("append history");
    OpenOptions::new()
        .append(true)
        .open(root.join("journal/vacation-history.log"))
        .and_then(|mut journal| journal.write_all(b"1\tpartial"))
        .expect("write partial tail");

    vacation_suppression::recover(&root).expect("recover tail");
    assert_eq!(
        vacation_suppression::load(&root, &committed)
            .expect("load")
            .len(),
        1
    );
    assert!(
        fs::read(root.join("journal/vacation-history.log"))
            .expect("journal")
            .ends_with(b"\n")
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn compaction_keeps_latest_committed_claim_and_drops_expired_claims() {
    let root = directory();
    fs::create_dir_all(root.join("journal")).expect("journal directory");
    let records = [
        record(41, "same", 500),
        record(42, "uncommitted", 900),
        record(41, "same", 900),
        record(41, "expired", 99),
    ];
    let mut journal = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(vacation_suppression::path(&root))
        .expect("journal");
    for entry in records {
        journal
            .write_all(&vacation_suppression::encode_record(&entry))
            .expect("write journal record");
    }
    journal.sync_all().expect("sync journal");
    let committed = HashSet::from([41]);

    vacation_compaction::compact(&root, &committed, 100, 0, 8 * 1024 * 1024)
        .expect("compact history");

    let history = vacation_suppression::load(&root, &committed).expect("load compacted history");
    assert_eq!(history.len(), 1);
    assert_eq!(history.get(&key("same")), Some(&900));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn compaction_drops_oldest_entries_but_retains_one_thousand() {
    let root = directory();
    fs::create_dir_all(root.join("journal")).expect("journal directory");
    let committed = HashSet::from([41]);
    let account = "b".repeat(256);
    let sender = "a".repeat(320);
    let suffix = "x".repeat(252);
    let mut journal = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(vacation_suppression::path(&root))
        .expect("journal");
    let mut line_bytes = None;
    for number in 0..1_002 {
        let entry = Record {
            transaction_id: 41,
            expires_at: 10_000,
            key: VacationKey {
                account: account.clone(),
                sender: sender.clone(),
                identity: format!("{number:04}{suffix}"),
            },
        };
        let line = vacation_suppression::encode_record(&entry);
        line_bytes = Some(line.len());
        journal.write_all(&line).expect("write history");
    }
    journal.sync_all().expect("sync history");
    drop(journal);
    let line_bytes = line_bytes.expect("at least one line");
    let maximum = u64::try_from(line_bytes * 1_000).expect("history size");

    vacation_compaction::compact(&root, &committed, 100, 0, maximum)
        .expect("compact to RFC minimum");

    let history = vacation_suppression::load(&root, &committed).expect("load compacted history");
    assert_eq!(history.len(), 1_000);
    assert!(!history.contains_key(&max_key("0000", &account, &sender, &suffix)));
    assert!(history.contains_key(&max_key("1001", &account, &sender, &suffix)));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn interrupted_replacement_restores_backup_and_prefers_published_snapshot() {
    let root = directory();
    let journal = root.join("journal");
    fs::create_dir_all(&journal).expect("journal directory");
    let original = vacation_suppression::encode_record(&record(41, "recovered", 500));
    fs::write(journal.join("vacation-history.compact.bak"), &original).expect("backup");
    fs::write(journal.join("vacation-history.compact.tmp"), b"partial").expect("temp");

    vacation_compaction::recover_replacement(&root).expect("restore backup");

    assert_eq!(
        fs::read(vacation_suppression::path(&root)).expect("restored"),
        original
    );
    assert!(!journal.join("vacation-history.compact.tmp").exists());
    fs::write(journal.join("vacation-history.compact.bak"), b"old").expect("old backup");
    fs::write(journal.join("vacation-history.compact.tmp"), b"partial").expect("new temp");
    vacation_compaction::recover_replacement(&root).expect("keep published snapshot");
    assert_eq!(
        fs::read(vacation_suppression::path(&root)).expect("snapshot"),
        original
    );
    fs::remove_dir_all(root).expect("cleanup");
}

fn record(transaction_id: u64, identity: &str, expires_at: u64) -> Record {
    Record {
        transaction_id,
        expires_at,
        key: key(identity),
    }
}

fn key(identity: &str) -> VacationKey {
    VacationKey {
        account: "bob".into(),
        sender: "alice@peer.test".into(),
        identity: identity.into(),
    }
}

fn max_key(prefix: &str, account: &str, sender: &str, suffix: &str) -> VacationKey {
    VacationKey {
        account: account.into(),
        sender: sender.into(),
        identity: format!("{prefix}{suffix}"),
    }
}

fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-vacation-history-{nonce}"))
}
