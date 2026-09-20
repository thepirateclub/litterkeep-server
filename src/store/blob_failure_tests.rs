//! Низкоуровневая матрица ошибок публикации immutable message blob.

use super::{Store, blobs};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn directory(failure: blobs::PublishFailure) -> PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-blob-{failure:?}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn failed_blob_operations_recover_without_blocking_next_commit() {
    let failures = [
        blobs::PublishFailure::TempCreate,
        blobs::PublishFailure::ShortWrite,
        blobs::PublishFailure::FileSync,
        blobs::PublishFailure::Rename,
        blobs::PublishFailure::DirectorySync,
    ];
    for failure in failures {
        verify_failure(failure);
    }
}

fn verify_failure(failure: blobs::PublishFailure) {
    let root = directory(failure);
    let store = Store::open(&root).expect("store");
    let name = "message-1.eml";
    let result = blobs::publish_failing(&root, name, b"partial payload", failure);
    assert!(result.is_err());
    drop(store);

    let final_path = root.join("blobs").join(name);
    let renamed = matches!(failure, blobs::PublishFailure::DirectorySync);
    assert_eq!(final_path.exists(), renamed);

    let mut reopened = Store::open(&root).expect("recovery");
    assert_no_temporary_blob(&root);
    let uid = reopened
        .append("bob", b"committed after recovery")
        .expect("append");
    assert_eq!(uid, 1);
    let messages = reopened.messages("bob").expect("messages");
    let message = messages.first().expect("committed message");
    assert_eq!(message.bytes, b"committed after recovery");
    drop(reopened);
    fs::remove_dir_all(root).expect("cleanup");
}

fn assert_no_temporary_blob(root: &Path) {
    let has_temporary = fs::read_dir(root.join("blobs"))
        .expect("blob directory")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .any(|name| {
            matches!(
                (name.starts_with('.'), name.ends_with(".tmp")),
                (true, true)
            )
        });
    assert!(!has_temporary);
}
