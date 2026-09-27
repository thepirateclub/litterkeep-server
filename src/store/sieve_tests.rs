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
            .put_sieve_script(
                "bob",
                "invoices",
                "require [\"unknown-extension\"]; keep;".into()
            )
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

#[test]
fn script_count_limit_preserves_typed_quota_error() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    for number in 0..16 {
        store
            .put_sieve_script("bob", &format!("script-{number}"), "keep;".into())
            .expect("script within quota");
    }
    let error = store
        .put_sieve_script("bob", "script-over-limit", "keep;".into())
        .expect_err("seventeenth script exceeds quota");
    assert!(sieve_scripts::is_quota_error(&error));
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn missing_script_preserves_typed_not_found_cause() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    let error = store
        .activate_sieve_script("bob", Some("absent"))
        .expect_err("unknown script cannot become active");
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(sieve_scripts::is_missing_error(&error));
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn active_script_deletion_preserves_typed_active_cause() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    store
        .put_sieve_script("bob", "route", "keep;".into())
        .expect("put script");
    store
        .activate_sieve_script("bob", Some("route"))
        .expect("activate script");
    let error = store
        .delete_sieve_script("bob", "route")
        .expect_err("active script cannot be deleted");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(sieve_scripts::is_active_error(&error));
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rename_collision_is_typed_and_active_rename_survives_reopen() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    store
        .put_sieve_script("bob", "route", "keep;".into())
        .expect("put route");
    store
        .put_sieve_script("bob", "target", "discard;".into())
        .expect("put target");
    let collision = store
        .rename_sieve_script("bob", "route", "target")
        .expect_err("rename must not overwrite existing target");
    assert_eq!(collision.kind(), std::io::ErrorKind::AlreadyExists);
    assert!(sieve_scripts::is_already_exists_error(&collision));
    store
        .activate_sieve_script("bob", Some("route"))
        .expect("activate source");
    store
        .rename_sieve_script("bob", "route", "route-renamed")
        .expect("rename active script");
    assert_eq!(
        store.sieve_scripts("bob"),
        vec![
            ScriptInfo {
                name: "route-renamed".into(),
                version: 1,
                active: true,
            },
            ScriptInfo {
                name: "target".into(),
                version: 2,
                active: false,
            },
        ]
    );
    drop(store);
    let store = Store::open(&root).expect("reopen");
    assert!(
        store
            .sieve_scripts("bob")
            .iter()
            .any(|script| { script.name == "route-renamed" && script.active })
    );
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}
