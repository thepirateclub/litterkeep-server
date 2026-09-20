//! Проверка повторяемости bootstrap без смены секретов и PHC records.

use super::initialize;
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
fn saved_passwords_authenticate_all_four_accounts() {
    let root = directory();
    let report = initialize(&root).expect("init");
    let credentials = fs::read_to_string(report.credentials_path).expect("credentials");
    let accounts = Accounts::open(&root).expect("accounts");
    for line in credentials.lines() {
        let mut fields = line.split('\t');
        let name = fields.next().expect("name");
        let password = fields.next().expect("password");
        assert!(accounts.authenticate(name, password.as_bytes()));
    }
    fs::remove_dir_all(root).expect("cleanup");
}
