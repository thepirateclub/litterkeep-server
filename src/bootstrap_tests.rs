//! Проверка повторяемости bootstrap без смены секретов и PHC records.

use super::initialize;
use super::initialize_smtp_utf8_test;
use crate::accounts::Accounts;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn directory() -> PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-bootstrap-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn repeated_initialize_preserves_secrets_and_accounts() {
    let root = directory();
    let first = initialize(&root).expect("first init");
    assert_eq!(first.created_accounts, 4);
    let secrets = fs::read(&first.credentials_path).expect("secrets");
    let account_files = fs::read_dir(root.join("accounts"))
        .expect("accounts")
        .map(|entry| {
            let path = entry.expect("entry").path();
            (path.clone(), fs::read(path).expect("PHC"))
        })
        .collect::<Vec<_>>();

    let second = initialize(&root).expect("second init");
    assert_eq!(second.created_accounts, 0);
    assert_eq!(second.existing_accounts, 4);
    assert_eq!(
        fs::read(&second.credentials_path).expect("secrets"),
        secrets
    );
    for (path, phc) in account_files {
        assert_eq!(fs::read(path).expect("same PHC"), phc);
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn saved_passwords_authenticate_all_acceptance_accounts() {
    let root = directory();
    let report = initialize(&root).expect("init");
    let credentials = fs::read_to_string(report.credentials_path).expect("credentials");
    let accounts = Accounts::open(&root).expect("accounts");
    for line in credentials.lines() {
        let mut fields = line.split('\t');
        let name = fields.next().expect("name");
        let password = fields.next().expect("password");
        assert!(password.chars().count() <= 40, "POP3 PASS limit for {name}");
        assert!(
            accounts
                .authenticate(name, password.as_bytes())
                .expect("verify account")
        );
    }
    assert!(!accounts.exists("пользователь"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn unicode_acceptance_account_is_opt_in_and_preserves_existing_credentials() {
    let root = directory();
    let standard = initialize(&root).expect("standard init");
    let original_credentials = fs::read(&standard.credentials_path).expect("standard secrets");
    let first = initialize_smtp_utf8_test(&root).expect("Unicode test account init");
    assert!(first.created);
    let unicode_secrets = fs::read_to_string(&first.credentials_path).expect("Unicode secrets");
    let mut fields = unicode_secrets.trim_end().split('\t');
    let username = fields.next().expect("Unicode username");
    let password = fields.next().expect("Unicode password");
    assert_eq!(username, "пользователь");
    assert!(fields.next().is_none());

    let accounts = Accounts::open(&root).expect("account store");
    assert!(
        accounts
            .authenticate(username, password.as_bytes())
            .expect("verify account")
    );
    let second = initialize_smtp_utf8_test(&root).expect("repeat Unicode init");
    assert!(!second.created);
    assert_eq!(
        fs::read(&standard.credentials_path).expect("unchanged standard secrets"),
        original_credentials
    );
    assert_eq!(
        fs::read(&second.credentials_path).expect("unchanged Unicode secret"),
        unicode_secrets.as_bytes()
    );
    fs::remove_dir_all(root).expect("cleanup");
}
