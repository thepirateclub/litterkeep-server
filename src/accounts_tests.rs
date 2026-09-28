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
    assert!(accounts.authenticate("alice", b"a").expect("verify"));
    assert!(
        accounts
            .authenticate("alice", "\u{00AA}".as_bytes())
            .expect("verify")
    );
    assert!(!accounts.authenticate("alice", b"A").expect("verify"));
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
fn legacy_phc_login_migrates_without_removing_the_original_hash() {
    let root = temporary_root("accounts-legacy");
    let accounts = Accounts::open(&root).expect("account store");
    let legacy_path = file_key::prepare(&root.join("accounts"), "bob", FileKind::Credentials)
        .expect("legacy credential path");
    let hash = legacy_hash("\u{00AA}".as_bytes());
    fs::write(&legacy_path, format!("{hash}\n")).expect("write legacy PHC fixture");

    assert!(accounts.exists("bob"));
    assert!(
        accounts
            .authenticate("bob", "\u{00AA}".as_bytes())
            .expect("verify")
    );
    assert!(accounts.create("bob", b"replacement").is_err());
    let migrated = file_key::path(&root.join("accounts"), "bob", FileKind::SaslPrepCredentials);
    let scram = file_key::path(&root.join("accounts"), "bob", FileKind::ScramCredentials);
    assert!(migrated.is_file());
    assert!(scram.is_file());
    assert!(
        accounts
            .authenticate("bob", b"a")
            .expect("verify normalized migration")
    );
    assert!(file_key::path(&root.join("accounts"), "bob", FileKind::Credentials).is_file());
    fs::remove_dir_all(root).expect("remove test account data");
}

#[test]
fn existing_saslprep_account_gets_scram_record_only_after_correct_login() {
    let root = temporary_root("accounts-scram-backfill");
    let accounts = Accounts::open(&root).expect("account store");
    let path = file_key::prepare(
        &root.join("accounts"),
        "erin",
        FileKind::SaslPrepCredentials,
    )
    .expect("normalized credential path");
    fs::write(&path, format!("{}\n", legacy_hash(b"stored-password")))
        .expect("write preexisting PHC2 fixture");
    let scram_path = file_key::path(&root.join("accounts"), "erin", FileKind::ScramCredentials);

    assert!(
        accounts
            .scram_verifier("erin")
            .expect("read record")
            .is_none()
    );
    assert!(
        !accounts
            .authenticate("erin", b"wrong-password")
            .expect("reject login")
    );
    assert!(!scram_path.exists());
    assert!(
        accounts
            .authenticate("erin", b"stored-password")
            .expect("accept login")
    );
    assert!(
        accounts
            .scram_verifier("erin")
            .expect("read backfill")
            .is_some()
    );
    assert!(
        accounts
            .authenticate("erin", b"stored-password")
            .expect("repeat login")
    );
    fs::remove_dir_all(root).expect("remove test account data");
}

#[test]
fn concurrent_legacy_logins_share_the_published_migration() {
    let root = temporary_root("accounts-legacy-race");
    Accounts::open(&root).expect("account store");
    let path = file_key::prepare(&root.join("accounts"), "dana", FileKind::Credentials)
        .expect("legacy credential path");
    fs::write(&path, format!("{}\n", legacy_hash(b"race-password")))
        .expect("write legacy PHC fixture");
    let workers = (0..4)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || {
                Accounts::open(root)
                    .expect("open account store")
                    .authenticate("dana", b"race-password")
                    .expect("authenticate concurrent legacy login")
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        assert!(worker.join().expect("join migration worker"));
    }
    assert!(
        file_key::path(
            &root.join("accounts"),
            "dana",
            FileKind::SaslPrepCredentials
        )
        .is_file()
    );
    assert!(path.is_file());
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

    assert!(
        accounts
            .authenticate("carol", "\u{00AA}".as_bytes())
            .expect("verify")
    );
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
