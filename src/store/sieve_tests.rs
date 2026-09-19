//! Долговечность и атомарность versioned Sieve scripts.

use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn directory() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-sieve-store-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[test]
fn active_script_survives_restart_and_invalid_update_keeps_old_version() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    let first = store
        .put_sieve_script(
            "bob",
            "invoices",
            "require [\"fileinto\"]; fileinto \"Invoices\";".into(),
        )
        .expect("put");
    store
        .activate_sieve_script("bob", Some("invoices"))
        .expect("active");
    assert!(
        store
            .put_sieve_script("bob", "invoices", "require [\"vacation\"]; keep;".into())
            .is_err()
    );
    assert_eq!(
        store.active_sieve_script("bob").expect("active source"),
        Some((
            first.version,
            "require [\"fileinto\"]; fileinto \"Invoices\";".into()
        ))
    );
    drop(store);
    let store = Store::open(&root).expect("reopen");
    assert_eq!(
        store.sieve_scripts("bob"),
        vec![ScriptInfo {
            name: "invoices".into(),
            version: first.version,
            active: true
        }]
    );
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}
