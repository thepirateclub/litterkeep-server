//! Проверки атомарности и восстановления пакетного APPEND в файловом Store.

use super::{AppendMetadata, AppendStage, Store, StoreFailpoint, WriteState, journal};
use std::fs;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static RUN: AtomicU64 = AtomicU64::new(0);

fn directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-multiappend-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ))
}

fn messages() -> Vec<(Vec<u8>, AppendMetadata)> {
    [b"first message".as_slice(), b"second message".as_slice()]
        .into_iter()
        .map(|bytes| (bytes.to_vec(), AppendMetadata::default()))
        .collect()
}

#[test]
fn batch_uses_ordered_uids_and_one_durable_commit_after_reopen() {
    let root = directory();
    let mut store = Store::open(&root).expect("store");
    assert_eq!(
        store.append_batch("bob", &messages()).expect("batch"),
        [1, 2]
    );
    drop(store);

    let mut reopened = Store::open(&root).expect("reopen");
    let snapshot = reopened.snapshot("bob").expect("snapshot");
    let uids = snapshot
        .messages
        .iter()
        .map(|message| message.uid)
        .collect::<Vec<_>>();
    let bodies = snapshot
        .messages
        .iter()
        .map(|message| message.bytes.as_slice())
        .collect::<Vec<_>>();
    assert_eq!(uids, [1, 2]);
    assert_eq!(
        bodies,
        [b"first message".as_slice(), b"second message".as_slice()]
    );
    let records = journal::records("bob", &root.join("mailboxes/bob.journal")).expect("journal");
    let transactions = records
        .iter()
        .map(|record| record.transaction_id)
        .collect::<std::collections::BTreeSet<_>>();
    let blobs = records
        .iter()
        .map(|record| record.blob_transaction_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(transactions.len(), 1);
    assert_eq!(blobs.len(), 2);
    assert_eq!(
        reopened
            .append("bob", b"after reopen")
            .expect("next append"),
        3
    );
    assert_eq!(
        reopened
            .snapshot("bob")
            .expect("updated snapshot")
            .messages
            .len(),
        3
    );
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn failure_after_first_mailbox_frame_replays_none_of_the_batch() {
    let root = directory();
    let mut store = Store::open(&root).expect("store");
    store.set_failpoint(StoreFailpoint::Stage(AppendStage::FirstMailboxFrame));
    let error = store
        .append_batch("bob", &messages())
        .expect_err("injected failure");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(store.write_state, WriteState::Poisoned);
    drop(store);

    let reopened = Store::open(&root).expect("reopen");
    assert!(
        reopened
            .snapshot("bob")
            .expect("snapshot")
            .messages
            .is_empty()
    );
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn empty_payload_cannot_be_used_as_an_append_cancellation() {
    let root = directory();
    let mut store = Store::open(&root).expect("store");
    let empty = [(Vec::new(), AppendMetadata::default())];
    let error = store
        .append_batch("bob", &empty)
        .expect_err("empty message");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(store.snapshot("bob").expect("snapshot").messages.is_empty());
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn batch_preserves_each_messages_independent_metadata_after_reopen() {
    let root = directory();
    let first_date = crate::mail_date::MailDate::from_unix_seconds(1_700_000_000);
    let second_date = crate::mail_date::MailDate::from_unix_seconds(1_700_000_100);
    let first_keyword = crate::mail_flags::KeywordFlag::parse("invoice").expect("keyword");
    let second_keyword = crate::mail_flags::KeywordFlag::parse("receipt").expect("keyword");
    let messages = vec![
        (
            b"first".to_vec(),
            AppendMetadata {
                flags: crate::mail_flags::MessageFlags::from_flags([
                    crate::mail_flags::SystemFlag::Seen,
                ]),
                keywords: [first_keyword.clone()].into_iter().collect(),
                internal_date: first_date,
            },
        ),
        (
            b"second".to_vec(),
            AppendMetadata {
                flags: crate::mail_flags::MessageFlags::from_flags([
                    crate::mail_flags::SystemFlag::Draft,
                ]),
                keywords: [second_keyword.clone()].into_iter().collect(),
                internal_date: second_date,
            },
        ),
    ];
    let mut store = Store::open(&root).expect("store");
    assert_eq!(store.append_batch("bob", &messages).expect("batch"), [1, 2]);
    drop(store);

    let store = Store::open(&root).expect("reopen");
    let snapshot = store.snapshot("bob").expect("snapshot");
    let first = snapshot.messages.first().expect("first message");
    let second = snapshot.messages.get(1).expect("second message");
    assert!(first.flags.contains(crate::mail_flags::SystemFlag::Seen));
    assert_eq!(first.keywords, [first_keyword].into_iter().collect());
    assert_eq!(first.internal_date, first_date);
    assert!(second.flags.contains(crate::mail_flags::SystemFlag::Draft));
    assert_eq!(second.keywords, [second_keyword].into_iter().collect());
    assert_eq!(second.internal_date, second_date);
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}
