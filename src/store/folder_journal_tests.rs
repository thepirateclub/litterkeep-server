//! Crash-тесты append-only журнала каталога папок.

use super::Store;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-folder-tail-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn incomplete_mutation_tail_is_trimmed_without_publication() {
    let root = directory();
    let mut store = Store::open(&root).expect("store");
    store.create_folder("bob", "Stable").expect("create");
    drop(store);

    let journal = root.join("folders").join("bob.journal");
    let committed_length = fs::metadata(&journal).expect("metadata").len();
    let mut file = OpenOptions::new()
        .append(true)
        .open(&journal)
        .expect("journal");
    file.write_all(b"LKFR\0\x01\0\x06Stable")
        .expect("partial rename");
    file.sync_all().expect("sync partial");
    drop(file);

    let mut recovered = Store::open(&root).expect("recover");
    assert!(recovered.resolve_folder("bob", "Stable").is_ok());
    assert_eq!(
        fs::metadata(&journal).expect("trimmed metadata").len(),
        committed_length
    );
    drop(recovered);
    fs::remove_dir_all(root).expect("cleanup");
}
