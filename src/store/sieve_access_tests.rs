//! Атомарность Sieve delivery plan, mailbox links и outbound queue после replay.

use super::super::{AppendStage, SieveDiagnostic, Store, StoreFailpoint};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn invoice_plan_delivers_to_invoices_and_default_goes_to_inbox() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    store
        .put_sieve_script("bob", "invoices", "require [\"fileinto\"]; if header :contains \"subject\" \"[invoice]\" { fileinto \"Invoices\"; stop; }".into())
        .expect("put");
    store
        .activate_sieve_script("bob", Some("invoices"))
        .expect("activate");
    deliver(&mut store, b"Subject: [invoice] paid\r\n\r\nbody");
    deliver(&mut store, b"Subject: ordinary\r\n\r\nbody");
    assert_eq!(folder_count(&mut store, "Invoices"), 1);
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    close(root, store);
}

/// RFC 5228 §2.10.3: повторная цель fileinto не дублирует mailbox delivery.
#[test]
fn repeated_fileinto_to_the_same_mailbox_is_not_an_error_or_duplicate() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(
        &mut store,
        "duplicate-folder",
        "require \"fileinto\"; fileinto \"Invoices\"; fileinto \"Invoices\";",
    );
    deliver(&mut store, b"Subject: duplicate folder\r\n\r\nbody");
    assert_eq!(folder_count(&mut store, "Invoices"), 1);
    assert_eq!(folder_count(&mut store, "INBOX"), 0);
    drop(store);
    let mut store = Store::open(&root).expect("reopen");
    assert_eq!(folder_count(&mut store, "Invoices"), 1);
    close(root, store);
}

#[test]
fn redirect_cancels_implicit_keep_and_preserves_envelope_sender() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store, "forward", "redirect \"remote@PEER.TEST\";");
    let transaction_id = deliver(&mut store, b"Subject: redirect\r\n\r\nbody");
    let expected = crate::queue::OwnedDelivery {
        transaction_id,
        attempts: 0,
        next_attempt_epoch: 0,
        sender: "alice@litterkeep.test".into(),
        recipient: "remote@peer.test".into(),
        original_recipient_index: Some(0),
        dsn_metadata: None,
        state: crate::queue::State::Pending,
    };
    assert_eq!(store.pending_outbound().expect("owned"), vec![expected]);
    let delivery = store
        .pending_outbound()
        .expect("pending")
        .into_iter()
        .next()
        .expect("job");
    assert!(
        store
            .outbound_blob(&delivery)
            .expect("blob")
            .ends_with(b"body")
    );
    store
        .transition_outbound(&delivery, crate::queue::State::Delivered)
        .expect("transition");
    assert!(store.pending_outbound().expect("delivered").is_empty());
    assert_eq!(folder_count(&mut store, "INBOX"), 0);
    drop(store);
    let store = Store::open(&root).expect("reopen");
    assert!(store.pending_outbound().expect("recovered").is_empty());
    close(root, store);
}

#[test]
fn explicit_keep_combines_local_delivery_with_redirect() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(
        &mut store,
        "keep-and-forward",
        "keep; redirect \"remote@peer.test\";",
    );
    deliver(&mut store, b"Subject: explicit keep\r\n\r\nbody");
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    assert_eq!(store.pending_outbound().expect("outbound").len(), 1);
    drop(store);
    let mut store = Store::open(&root).expect("reopen");
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    assert_eq!(store.pending_outbound().expect("recovered").len(), 1);
    close(root, store);
}

#[test]
fn runtime_error_keeps_mail_and_publishes_versioned_diagnostic() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    let info = store
        .put_sieve_script("bob", "too-many-redirects", redirect_limit_script())
        .expect("put");
    store
        .activate_sieve_script("bob", Some("too-many-redirects"))
        .expect("activate");
    let transaction_id = deliver(&mut store, b"Subject: bounded failure\r\n\r\nbody");
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    assert!(store.pending_outbound().expect("outbound").is_empty());
    assert_eq!(
        store.sieve_diagnostics("bob").expect("diagnostics"),
        vec![SieveDiagnostic {
            transaction_id,
            account: "bob".into(),
            script_version: info.version,
            error: crate::sieve::ExecutionError::RedirectLimit,
        }]
    );
    assert_eq!(
        store
            .sieve_diagnostics("bob")
            .expect("diagnostics")
            .first()
            .map(SieveDiagnostic::outcome),
        Some(crate::store::SieveFailureOutcome::ImplicitKeepOnly)
    );
    drop(store);
    let mut store = Store::open(&root).expect("reopen");
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    assert_eq!(store.sieve_diagnostics("bob").expect("recovered").len(), 1);
    close(root, store);
}

#[test]
fn interrupted_delivery_does_not_publish_sieve_diagnostic() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store, "failure", &redirect_limit_script());
    store.set_failpoint(StoreFailpoint::Stage(AppendStage::FirstMailboxFrame));
    assert!(
        store
            .deliver_sieve_many(
                &["bob".into()],
                "alice@litterkeep.test",
                &["bob@litterkeep.test".into()],
                b"Subject: interrupted\r\n\r\nbody",
            )
            .is_err()
    );
    drop(store);
    let mut store = Store::open(&root).expect("reopen");
    assert_eq!(folder_count(&mut store, "INBOX"), 0);
    assert!(
        store
            .sieve_diagnostics("bob")
            .expect("diagnostics")
            .is_empty()
    );
    close(root, store);
}

#[test]
fn redirect_discard_publishes_queue_without_an_inbox_link() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(
        &mut store,
        "redirect-only",
        "redirect \"remote@peer.test\"; discard;",
    );
    deliver(&mut store, b"Subject: redirect-only\r\n\r\nbody");
    assert_eq!(folder_count(&mut store, "INBOX"), 0);
    assert_eq!(store.pending_outbound().expect("outbound").len(), 1);
    drop(store);
    let mut store = Store::open(&root).expect("reopen");
    assert_eq!(folder_count(&mut store, "INBOX"), 0);
    assert_eq!(store.pending_outbound().expect("recovered").len(), 1);
    close(root, store);
}

#[test]
fn duplicate_recipient_executes_sieve_plan_once() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store, "forward", "redirect \"remote@peer.test\";");
    store
        .deliver_sieve_many(
            &["bob".into(), "bob".into()],
            "alice@litterkeep.test",
            &["bob@test".into(), "bob@litterkeep.test".into()],
            b"Subject: duplicate recipient\r\n\r\nbody",
        )
        .expect("deliver");
    assert_eq!(store.pending_outbound().expect("outbound").len(), 1);
    assert_eq!(folder_count(&mut store, "INBOX"), 0);
    close(root, store);
}

#[test]
fn denied_redirect_route_falls_back_to_keep_with_diagnostic() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(
        &mut store,
        "local-loop",
        "redirect \"bob@litterkeep.test\";",
    );
    deliver(&mut store, b"Subject: denied route\r\n\r\nbody");
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    assert!(store.pending_outbound().expect("outbound").is_empty());
    assert_eq!(
        store
            .sieve_diagnostics("bob")
            .expect("diagnostic")
            .last()
            .map(|value| value.error),
        Some(crate::sieve::ExecutionError::RedirectRouteDenied)
    );
    close(root, store);
}

#[test]
fn redirect_hop_limit_falls_back_to_keep_without_queue_job() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store, "forward", "redirect \"remote@peer.test\";");
    let mut message = String::new();
    for number in 0..8 {
        message.push_str(&format!("Received: by relay-{number}\r\n"));
    }
    message.push_str("Subject: loop guard\r\n\r\nbody");
    deliver(&mut store, message.as_bytes());
    assert_eq!(folder_count(&mut store, "INBOX"), 1);
    assert!(store.pending_outbound().expect("outbound").is_empty());
    assert_eq!(
        store
            .sieve_diagnostics("bob")
            .expect("diagnostic")
            .last()
            .map(|value| value.error),
        Some(crate::sieve::ExecutionError::RedirectLoop)
    );
    close(root, store);
}

fn activate(store: &mut Store, name: &str, source: &str) {
    store
        .put_sieve_script("bob", name, source.into())
        .expect("put");
    store
        .activate_sieve_script("bob", Some(name))
        .expect("activate");
}

fn deliver(store: &mut Store, message: &[u8]) -> u64 {
    store
        .deliver_sieve_many(
            &["bob".into()],
            "alice@litterkeep.test",
            &["bob@litterkeep.test".into()],
            message,
        )
        .expect("deliver")
}

fn folder_count(store: &mut Store, folder: &str) -> usize {
    store
        .snapshot_folder("bob", folder)
        .expect("snapshot")
        .snapshot
        .messages
        .len()
}

fn redirect_limit_script() -> String {
    (0..17)
        .map(|number| format!("redirect \"remote-{number}@peer.test\";"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn close(root: std::path::PathBuf, store: Store) {
    drop(store);
    fs::remove_dir_all(root).expect("cleanup");
}

fn directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-sieve-delivery-{nonce}"))
}

#[path = "sieve_origin_queue_tests.rs"]
mod origin_queue_tests;
