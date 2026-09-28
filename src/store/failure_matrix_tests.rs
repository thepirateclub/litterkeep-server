//! Replay matrix ошибок на последовательных границах append transaction.

use super::{AppendStage, Store, StoreFailpoint, WriteState};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn directory(stage: AppendStage) -> PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-failure-{stage:?}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn committed_before_error(stage: AppendStage) -> bool {
    matches!(
        stage,
        AppendStage::CommitFrameSynced
            | AppendStage::CommitDirectorySynced
            | AppendStage::BeforeMemoryPublish
    )
}

#[test]
fn append_failure_matrix_has_atomic_replay_boundary() {
    let stages = [
        AppendStage::BlobPublished,
        AppendStage::QueueWritten,
        AppendStage::DiagnosticsWritten,
        AppendStage::EnvelopeWritten,
        AppendStage::DsnReportWritten,
        AppendStage::DsnSuccessWritten,
        AppendStage::DsnSuccessReportWritten,
        AppendStage::VacationWritten,
        AppendStage::FirstMailboxFrame,
        AppendStage::MailboxDirectorySynced,
        AppendStage::RecentWritten,
        AppendStage::CommitFrameSynced,
        AppendStage::CommitDirectorySynced,
        AppendStage::BeforeMemoryPublish,
    ];
    for stage in stages {
        verify_stage(stage);
    }
}

fn verify_stage(stage: AppendStage) {
    let root = directory(stage);
    let mut store = Store::open(&root).expect("store");
    store.set_failpoint(StoreFailpoint::Stage(stage));
    let error = store
        .append("bob", b"failure matrix")
        .expect_err("injected");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(store.write_state, WriteState::Poisoned);
    assert!(store.append("bob", b"forbidden continuation").is_err());
    drop(store);

    let reopened = Store::open(&root).expect("replay");
    let messages = reopened.messages("bob").expect("messages");
    assert_eq!(messages.len(), usize::from(committed_before_error(stage)));
    if let Some(message) = messages.first() {
        assert_eq!(message.bytes, b"failure matrix");
    }
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}
