use super::*;
use std::fs;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
static RUN: AtomicU64 = AtomicU64::new(0);
fn dir() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "litterkeep-store-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        RUN.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&path);
    path
}
fn record(uid: u64, transaction_id: u64) -> model::Record {
    model::Record {
        uid,
        transaction_id,
        name: format!("message-{transaction_id}.eml"),
        flags: MessageFlags::default(),
        internal_date: crate::mail_date::MailDate::from_unix_seconds(0),
    }
}
#[test]
fn restart_and_second_writer() {
    let p = dir();
    let mut s = Store::open(&p).unwrap();
    assert_eq!(s.append("bob", b"bytes").unwrap(), 1);
    assert_eq!(
        Store::open(&p).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    drop(s);
    let s = Store::open(&p).unwrap();
    let messages = s.messages("bob").unwrap();
    let message = messages.first().expect("stored message");
    assert_eq!(message.bytes, b"bytes");
    drop(s);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn initial_metadata_survives_restart_in_mailbox_frame() {
    let p = dir();
    let mut store = Store::open(&p).expect("store");
    let flags = MessageFlags::from_flags([
        crate::mail_flags::SystemFlag::Seen,
        crate::mail_flags::SystemFlag::Draft,
    ]);
    let metadata = AppendMetadata {
        flags,
        internal_date: crate::mail_date::MailDate::from_unix_seconds(1_790_000_000),
    };
    store
        .append_with_metadata("bob", b"metadata", metadata)
        .expect("append");
    drop(store);
    let reopened = Store::open(&p).expect("reopen");
    let messages = reopened.messages("bob").expect("messages");
    let message = messages.first().expect("message");
    assert_eq!(message.flags, flags);
    assert_eq!(message.internal_date, metadata.internal_date);
    drop(reopened);
    let _ = fs::remove_dir_all(p);
}
#[test]
fn recovery_only_trims_incomplete_tail() {
    let p = dir();
    let mut s = Store::open(&p).unwrap();
    s.append("bob", b"x").unwrap();
    drop(s);
    let i = p.join("mailboxes/bob.journal");
    OpenOptions::new()
        .append(true)
        .open(&i)
        .unwrap()
        .write_all(b"LKJ")
        .unwrap();
    let s = Store::open(&p).unwrap();
    assert_eq!(s.messages("bob").unwrap().len(), 1);
    drop(s);
    let _ = fs::remove_dir_all(p);
}
#[test]
fn corruption_and_missing_blob_are_errors() {
    let p = dir();
    let mut s = Store::open(&p).unwrap();
    s.append("bob", b"x").unwrap();
    fs::remove_file(p.join("blobs/message-1.eml")).unwrap();
    assert_eq!(
        s.messages("bob").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    drop(s);
    let mut damaged = journal::frame(&record(1, 1));
    let checksum_byte = damaged
        .iter_mut()
        .rev()
        .nth(4)
        .expect("checksum byte before commit marker");
    *checksum_byte ^= 0xff;
    fs::write(p.join("mailboxes/bob.journal"), damaged).unwrap();
    assert_eq!(
        Store::open(&p).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    let _ = fs::remove_dir_all(p);
}

#[test]
fn next_uid_is_cached_and_recovered_once() {
    let p = dir();
    let mut store = Store::open(&p).expect("store");
    assert_eq!(store.append("bob", b"one").expect("first"), 1);
    assert_eq!(store.append("alice", b"other").expect("other mailbox"), 1);
    assert_eq!(store.append("bob", b"two").expect("second"), 2);
    drop(store);
    let mut reopened = Store::open(&p).expect("reopen");
    let before_append = reopened.snapshot("bob").expect("snapshot");
    assert_eq!(before_append.uid_validity, 1);
    assert_eq!(before_append.uid_next, 3);
    assert_eq!(reopened.append("bob", b"three").expect("third"), 3);
    let after_append = reopened.snapshot("bob").expect("snapshot");
    assert_eq!(after_append.uid_validity, before_append.uid_validity);
    assert_eq!(after_append.uid_next, 4);
    drop(reopened);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn unversioned_mail_data_is_not_guessed_or_rewritten() {
    let p = dir();
    fs::create_dir_all(p.join("mailboxes")).expect("mailboxes");
    fs::write(p.join("mailboxes/bob.journal"), b"legacy").expect("legacy journal");
    assert_eq!(
        Store::open(&p)
            .expect_err("migration must be explicit")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!p.join("format-version").exists());
    let _ = fs::remove_dir_all(p);
}

#[test]
fn blob_quota_rejects_before_publication() {
    let p = dir();
    let mut store = Store::open_with_quota(&p, 4).expect("small quota store");
    let error = store
        .append("bob", b"12345")
        .expect_err("oversized blob must be rejected");
    assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    assert!(store.messages("bob").expect("mailbox").is_empty());
    assert!(
        fs::read_dir(p.join("blobs"))
            .expect("blobs")
            .next()
            .is_none()
    );
    drop(store);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn multi_recipient_commit_shares_one_blob() {
    let p = dir();
    let mut store = Store::open(&p).expect("store");
    let recipients = vec!["bob".to_owned(), "alice".to_owned(), "bob".to_owned()];
    assert_eq!(
        store
            .append_many(&recipients, b"shared")
            .expect("atomic delivery"),
        1
    );
    let bob = store.messages("bob").expect("bob");
    let alice = store.messages("alice").expect("alice");
    let bob_message = bob.first().expect("bob message");
    let alice_message = alice.first().expect("alice message");
    assert_eq!(bob_message.path, alice_message.path);
    assert_eq!(fs::read_dir(p.join("blobs")).expect("blobs").count(), 1);
    drop(store);
    let reopened = Store::open(&p).expect("reopen");
    assert_eq!(reopened.messages("bob").expect("bob restart").len(), 1);
    assert_eq!(reopened.messages("alice").expect("alice restart").len(), 1);
    drop(reopened);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn mailbox_frame_without_global_commit_stays_invisible() {
    let p = dir();
    let store = Store::open(&p).expect("store");
    drop(store);
    blobs::publish(&p, "message-1.eml", b"orphan transaction").expect("blob");
    let journal_path = p.join("mailboxes/bob.journal");
    let mut journal_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(journal_path)
        .expect("journal");
    journal_file
        .write_all(&journal::frame(&record(1, 1)))
        .expect("uncommitted link");
    journal_file.sync_all().expect("journal sync");
    drop(journal_file);

    let mut recovered = Store::open(&p).expect("recover uncommitted transaction");
    assert!(recovered.messages("bob").expect("invisible").is_empty());
    assert_eq!(
        recovered.append("bob", b"committed").expect("new commit"),
        2
    );
    let visible = recovered.messages("bob").expect("visible");
    let message = visible.first().expect("committed message");
    assert_eq!(message.uid, 2);
    assert_eq!(message.bytes, b"committed");
    drop(recovered);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn partial_multi_recipient_write_poisoned_and_invisible() {
    let p = dir();
    let mut store = Store::open(&p).expect("store");
    store.set_failpoint(StoreFailpoint::AfterFirstMailboxFrame);
    let recipients = vec!["bob".to_owned(), "alice".to_owned()];
    assert!(store.append_many(&recipients, b"partial").is_err());
    let poisoned = store
        .append("bob", b"must not continue")
        .expect_err("uncertain writer must stop");
    assert_eq!(poisoned.kind(), io::ErrorKind::Other);
    assert!(store.messages("bob").expect("materialized view").is_empty());
    drop(store);

    let mut recovered = Store::open(&p).expect("recovery");
    assert!(recovered.messages("bob").expect("bob").is_empty());
    assert!(recovered.messages("alice").expect("alice").is_empty());
    assert_eq!(
        recovered.append("bob", b"after recovery").expect("append"),
        2
    );
    drop(recovered);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn flag_batch_is_durable_and_uid_addressed() {
    let p = dir();
    let mut store = Store::open(&p).expect("store");
    store.append("bob", b"one").expect("first");
    store.append("bob", b"two").expect("second");
    let initial = MessageFlags::from_flags([SystemFlag::Seen, SystemFlag::Flagged]);
    store
        .update_flags("bob", &[2, 1, 2], FlagMutation::Add, initial)
        .expect("flag batch");
    store
        .update_flags(
            "bob",
            &[2],
            FlagMutation::Remove,
            MessageFlags::from_flags([SystemFlag::Flagged]),
        )
        .expect("remove flag");
    drop(store);

    let reopened = Store::open(&p).expect("reopen");
    let messages = reopened.messages("bob").expect("messages");
    let first = messages.first().expect("first message");
    let second = messages.get(1).expect("second message");
    assert!(first.flags.contains(SystemFlag::Seen));
    assert!(first.flags.contains(SystemFlag::Flagged));
    assert!(second.flags.contains(SystemFlag::Seen));
    assert!(!second.flags.contains(SystemFlag::Flagged));
    drop(reopened);
    let _ = fs::remove_dir_all(p);
}

#[test]
fn flag_update_rejects_unknown_uid_before_writing() {
    let p = dir();
    let mut store = Store::open(&p).expect("store");
    store.append("bob", b"one").expect("message");
    let error = store
        .update_flags("bob", &[2], FlagMutation::Replace, MessageFlags::default())
        .expect_err("unknown UID");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(!p.join("mailboxes/bob.flags").exists());
    drop(store);
    let _ = fs::remove_dir_all(p);
}
