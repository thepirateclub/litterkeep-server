//! Проверяет Unicode alias resolution для сохранённых имён mailbox.

use super::{FolderCatalog, FolderInfo, storage_key};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn old_decomposed_name_resolves_lists_and_mutates_by_nfc_alias() {
    let root = temporary_root();
    drop(crate::store::Store::open(&root).expect("initialize store format"));
    let legacy = FolderInfo {
        id: 9,
        name: "cafe\u{0301}".to_owned(),
        special_use: None,
    };
    super::super::folder_journal::append_create(&root.join("folders"), "bob", &legacy)
        .expect("legacy journal");

    let mut catalog = FolderCatalog::load(&root).expect("load legacy folder catalog");
    let old_key = storage_key("bob", &legacy);
    assert_eq!(catalog.resolve("bob", "café").expect("NFC alias"), old_key);
    assert!(
        catalog
            .list("bob")
            .expect("folder listing")
            .iter()
            .any(|folder| folder.id == legacy.id && folder.name == "café")
    );
    catalog
        .rename("bob", "café", "Archive/2026")
        .expect("rename through canonical alias");
    assert_eq!(
        catalog.resolve("bob", "Archive/2026").expect("renamed"),
        old_key
    );
    catalog.delete("bob", "Archive/2026").expect("delete alias");
    assert!(catalog.resolve("bob", "Archive/2026").is_err());

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ambiguous_canonical_folder_names_fail_without_selecting_either_mailbox() {
    let root = temporary_root();
    std::fs::create_dir_all(root.join("folders")).expect("folder root");
    for (id, name) in [(19, "cafe\u{0301}"), (20, "café")] {
        let folder = FolderInfo {
            id,
            name: name.to_owned(),
            special_use: None,
        };
        super::super::folder_journal::append_create(&root.join("folders"), "bob", &folder)
            .expect("legacy folder journal");
    }
    let mut catalog = FolderCatalog::load(&root).expect("load colliding folder catalog");

    assert_eq!(
        catalog
            .resolve("bob", "café")
            .expect_err("canonical alias is ambiguous")
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn rename_cannot_create_a_second_canonical_folder_alias() {
    let root = temporary_root();
    std::fs::create_dir_all(root.join("folders")).expect("folder root");
    for (id, name) in [(19, "source"), (20, "cafe\u{0301}")] {
        let folder = FolderInfo {
            id,
            name: name.to_owned(),
            special_use: None,
        };
        super::super::folder_journal::append_create(&root.join("folders"), "bob", &folder)
            .expect("legacy folder journal");
    }
    let mut catalog = FolderCatalog::load(&root).expect("load catalog");

    assert_eq!(
        catalog
            .rename("bob", "source", "café")
            .expect_err("canonical duplicate is not allowed")
            .kind(),
        std::io::ErrorKind::AlreadyExists
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legacy_folder_messages_survive_store_reopen_and_resolve_by_nfc_name() {
    let root = temporary_root();
    drop(crate::store::Store::open(&root).expect("initialize store format"));
    let legacy = FolderInfo {
        id: 9,
        name: "cafe\u{0301}".to_owned(),
        special_use: None,
    };
    super::super::folder_journal::append_create(&root.join("folders"), "bob", &legacy)
        .expect("legacy journal");
    let mut store = crate::store::Store::open(&root).expect("open legacy store");
    let key = store
        .resolve_folder("bob", "café")
        .expect("resolve old folder through NFC");
    store
        .append(&key, b"durable legacy folder content")
        .expect("append");
    drop(store);

    let mut reopened = crate::store::Store::open(&root).expect("reopen store");
    let mailbox = reopened
        .snapshot_folder("bob", "café")
        .expect("reopen legacy folder through NFC");
    assert_eq!(
        mailbox
            .snapshot
            .messages
            .first()
            .map(|message| message.bytes.as_slice()),
        Some(b"durable legacy folder content".as_slice())
    );
    let _ = std::fs::remove_dir_all(root);
}

fn temporary_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-folder-nfc-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}
