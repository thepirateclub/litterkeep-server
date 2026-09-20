//! Атомарность Sieve delivery plan, mailbox links и outbound queue после replay.

use super::super::Store;
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

#[test]
fn redirect_cancels_implicit_keep_and_preserves_envelope_sender() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store, "forward", "redirect \"remote@test\";");
    let transaction_id = deliver(&mut store, b"Subject: redirect\r\n\r\nbody");
    let expected = crate::queue::OwnedDelivery {
        transaction_id,
        attempts: 0,
        next_attempt_epoch: 0,
        sender: "alice@litterkeep.test".into(),
        recipient: "remote@test".into(),
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
        .deliver_sieve_many(&["bob".into()], "alice@litterkeep.test", message)
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
