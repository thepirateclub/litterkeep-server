//! Границы ёмкости и recovery RFC 5230 response suppression journal.

use super::vacation_suppression::{self, PendingClaim, VacationKey};
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
    for number in 0..1_000 {
        let claim = PendingClaim {
            key: VacationKey {
                account: account.clone(),
                sender: sender.clone(),
                identity: format!("{number:04}{suffix}"),
            },
            expires_at: 10_000,
        };
        vacation_suppression::append(&root, 41, Some(&claim)).expect("append history");
    }

    let committed = HashSet::from([41]);
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
    };
    vacation_suppression::append(&root, 41, Some(&claim)).expect("append history");
    OpenOptions::new()
        .append(true)
        .open(root.join("journal/vacation-history.log"))
        .and_then(|mut journal| journal.write_all(b"1\tpartial"))
        .expect("write partial tail");

    vacation_suppression::recover(&root).expect("recover tail");
    let committed = HashSet::from([41]);
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

fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-vacation-history-{nonce}"))
}
