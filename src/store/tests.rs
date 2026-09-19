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
#[test]
fn restart_and_second_writer() {
    let p = dir();
    let s = Store::open(&p).unwrap();
    assert_eq!(s.append("bob", b"bytes").unwrap(), 1);
    assert_eq!(
        Store::open(&p).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    drop(s);
    let s = Store::open(&p).unwrap();
    assert_eq!(s.messages("bob").unwrap()[0].bytes, b"bytes");
    drop(s);
    let _ = fs::remove_dir_all(p);
}
#[test]
fn recovery_only_trims_incomplete_tail() {
    let p = dir();
    let s = Store::open(&p).unwrap();
    s.append("bob", b"x").unwrap();
    drop(s);
    let i = p.join("mailboxes/bob.journal");
    OpenOptions::new()
        .append(true)
        .open(&i)
        .unwrap()
        .write_all(b"V1 2 bob-2.eml")
        .unwrap();
    let s = Store::open(&p).unwrap();
    assert_eq!(s.messages("bob").unwrap().len(), 1);
    drop(s);
    let _ = fs::remove_dir_all(p);
}
#[test]
fn corruption_and_missing_blob_are_errors() {
    let p = dir();
    let s = Store::open(&p).unwrap();
    s.append("bob", b"x").unwrap();
    fs::remove_file(p.join("blobs/bob-1.eml")).unwrap();
    assert_eq!(
        s.messages("bob").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    drop(s);
    fs::write(p.join("mailboxes/bob.journal"), b"V1 1 bob-1.eml 0\n").unwrap();
    assert_eq!(
        Store::open(&p).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    let _ = fs::remove_dir_all(p);
}
