use super::super::Store;
use super::{FileKind, entries, path};
use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn legacy_ascii_path_remains_unchanged() {
    let root = std::env::temp_dir().join("litterkeep-file-key");
    assert_eq!(
        path(&root, "alice-folder-1", FileKind::Journal),
        root.join("alice-folder-1.journal")
    );
}

#[test]
fn encoded_utf8_keys_round_trip_without_path_separators() {
    let root = std::env::temp_dir().join(format!(
        "litterkeep-file-key-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let key = "用户/../почта";
    let encoded_path = path(&root, key, FileKind::Journal);
    let encoded_directory = root.join("utf8");
    assert_eq!(encoded_path.parent(), Some(encoded_directory.as_path()));
    fs::create_dir_all(&encoded_directory).unwrap();
    fs::write(&encoded_path, b"fixture").unwrap();
    let found = entries(&root, FileKind::Journal).unwrap();
    assert_eq!(found, vec![(key.to_owned(), encoded_path)]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn longest_mailbox_key_uses_bounded_filename_components() {
    let key = "用户".repeat(42);
    let path = path(&std::env::temp_dir(), &key, FileKind::Journal);
    assert!(
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.len() <= 255)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn long_unicode_key_recovers_across_hierarchical_hex_path() {
    let root = std::env::temp_dir().join(format!(
        "litterkeep-long-key-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let key = "用户".repeat(42);
    let encoded_path = path(&root, &key, FileKind::Journal);
    fs::create_dir_all(encoded_path.parent().unwrap()).unwrap();
    fs::write(&encoded_path, b"fixture").unwrap();
    assert_eq!(
        entries(&root, FileKind::Journal).unwrap(),
        vec![(key, encoded_path)]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unicode_mailbox_append_flags_expunge_and_recovery_are_durable() {
    let root = std::env::temp_dir().join(format!(
        "litterkeep-unicode-mailbox-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mailbox = "用户/../почта";
    let mut store = Store::open(&root).unwrap();
    store.append(mailbox, b"fixture").unwrap();
    store
        .update_flags(
            mailbox,
            &[1],
            FlagMutation::Add,
            MessageFlags::from_flags([SystemFlag::Deleted]),
        )
        .unwrap();
    drop(store);

    let mut recovered = Store::open(&root).unwrap();
    let snapshot = recovered.snapshot(mailbox).unwrap();
    assert_eq!(snapshot.messages.len(), 1);
    assert!(
        snapshot
            .messages
            .first()
            .is_some_and(|message| message.flags.contains(SystemFlag::Deleted))
    );
    assert_eq!(recovered.expunge(mailbox, &[1]).unwrap(), vec![1]);
    drop(recovered);

    let reopened = Store::open(&root).unwrap();
    assert!(reopened.snapshot(mailbox).unwrap().messages.is_empty());
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unicode_move_commits_encoded_destination_and_source_tombstone() {
    let root = std::env::temp_dir().join(format!(
        "litterkeep-unicode-move-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = "почта/Входящие";
    let destination = "почта/Архив";
    let mut store = Store::open(&root).unwrap();
    store.append(source, b"move fixture").unwrap();
    assert_eq!(store.move_messages(source, destination, &[1]).unwrap(), [1]);
    drop(store);

    let reopened = Store::open(&root).unwrap();
    assert!(reopened.snapshot(source).unwrap().messages.is_empty());
    assert_eq!(
        reopened
            .snapshot(destination)
            .unwrap()
            .messages
            .first()
            .map(|message| message.bytes.as_slice()),
        Some(b"move fixture".as_slice())
    );
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}
