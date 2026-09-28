//! Сквозные regression проверки исходной SMTP recipient связи в очереди.

use super::*;

/// Каждый outbound job сохраняет исходный RCPT до очереди и повторного open.
#[test]
fn redirects_keep_recipient_identity_when_queue_routes_are_identical() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    activate(&mut store, "bob-forward", "redirect \"remote@peer.test\";");
    store
        .put_sieve_script(
            "alice",
            "alice-forward",
            "redirect \"remote@peer.test\";".into(),
        )
        .expect("put Alice script");
    store
        .activate_sieve_script("alice", Some("alice-forward"))
        .expect("activate Alice script");
    store
        .deliver_sieve_many(
            &["bob".into(), "alice".into()],
            "sender@peer.test",
            &["bob@litterkeep.test".into(), "alice@litterkeep.test".into()],
            b"Subject: multi-recipient\r\n\r\nbody",
        )
        .expect("deliver");
    drop(store);

    let store = Store::open(&root).expect("reopen");
    let jobs = store.pending_outbound().expect("recovered jobs");
    assert_eq!(jobs.len(), 2);
    assert_eq!(
        jobs.first().and_then(|job| job.original_recipient_index),
        Some(0)
    );
    assert_eq!(
        jobs.get(1).and_then(|job| job.original_recipient_index),
        Some(1)
    );
    close(root, store);
}
