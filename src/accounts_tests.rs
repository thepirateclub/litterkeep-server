//! Проверки сохранения, проверки и совместимости Argon2 credentials.

use crate::accounts::Accounts;
use crate::auth::Credential;
use crate::store::file_key::{self, FileKind};
use argon2::{Argon2, password_hash::PasswordHasher};
use std::fs;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn new_account_password_uses_saslprep_credentials() {
    let root = temporary_root("accounts-saslprep");
    let accounts = Accounts::open(&root).expect("account store");
    accounts
        .create("alice", "\u{00AA}".as_bytes())
        .expect("create normalized account");
    assert!(accounts.exists("alice"));
    assert!(accounts.authenticate("alice", b"a"));
    assert!(accounts.authenticate("alice", "\u{00AA}".as_bytes()));
    assert!(!accounts.authenticate("alice", b"A"));
    assert_eq!(
        accounts
            .create("alice", b"replacement")
            .err()
            .map(|error| error.kind()),
        Some(io::ErrorKind::AlreadyExists)
    );
    let current = file_key::path(
        &root.join("accounts"),
        "alice",
        FileKind::SaslPrepCredentials,
    );
    let legacy = file_key::path(&root.join("accounts"), "alice", FileKind::Credentials);
    assert!(current.is_file());
    assert!(!legacy.exists());
    fs::remove_dir_all(root).expect("remove test account data");
}

#[test]
fn legacy_phc_accounts_remain_readable_without_relabeling_their_hashes() {
    let root = temporary_root("accounts-legacy");
    let accounts = Accounts::open(&root).expect("account store");
    let legacy_path = file_key::prepare(&root.join("accounts"), "bob", FileKind::Credentials)
        .expect("legacy credential path");
    let hash = legacy_hash("\u{00AA}".as_bytes());
    fs::write(&legacy_path, format!("{hash}\n")).expect("write legacy PHC fixture");

    assert!(accounts.exists("bob"));
    assert!(accounts.authenticate("bob", "\u{00AA}".as_bytes()));
    assert!(!accounts.authenticate("bob", b"a"));
    assert!(accounts.create("bob", b"replacement").is_err());
    assert!(!file_key::path(&root.join("accounts"), "bob", FileKind::SaslPrepCredentials).exists());
    fs::remove_dir_all(root).expect("remove test account data");
}

#[test]
fn existing_normalized_legacy_password_can_use_its_saslprep_equivalent() {
    let root = temporary_root("accounts-legacy-equivalent");
    let accounts = Accounts::open(&root).expect("account store");
    let path = file_key::prepare(&root.join("accounts"), "carol", FileKind::Credentials)
        .expect("legacy credential path");
    let hash = legacy_hash(b"a");
    fs::write(path, format!("{hash}\n")).expect("write legacy PHC fixture");

    assert!(accounts.authenticate("carol", "\u{00AA}".as_bytes()));
    fs::remove_dir_all(root).expect("remove test account data");
}

/// Создаёт случайно отделённый каталог для одного тестового credential owner.
fn temporary_root(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos()
    ))
}

/// Формирует старый raw-byte PHC fixture без нового SASLprep пути.
fn legacy_hash(password: &[u8]) -> String {
    Argon2::default()
        .hash_password(password)
        .expect("legacy PHC hash")
        .to_string()
}

/// Создание нормализованного PHC остаётся доступно для независимой проверки.
#[test]
fn credential_round_trip() {
    let credential = Credential::create(b"test-password").expect("create credential");
    assert!(credential.verify(b"test-password"));
    assert!(!credential.verify(b"wrong"));
    assert!(Credential::from_phc(credential.as_phc().to_owned()).is_ok());
}
