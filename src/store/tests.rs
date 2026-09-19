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
    fs::remove_file(p.join("blobs/bob-1.eml")).unwrap();
    assert_eq!(
        s.messages("bob").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    drop(s);
    let mut damaged = journal::frame(1, "bob-1.eml");
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
    assert_eq!(store.append("bob", b"two").expect("second"), 2);
    drop(store);
    let mut reopened = Store::open(&p).expect("reopen");
    assert_eq!(reopened.append("bob", b"three").expect("third"), 3);
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
