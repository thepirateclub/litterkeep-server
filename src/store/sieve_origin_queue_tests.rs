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
        jobs.first().map(|job| job.original_recipient_indices.as_slice()),
        Some([0].as_slice())
    );
    assert_eq!(
        jobs.get(1).map(|job| job.original_recipient_indices.as_slice()),
        Some([1].as_slice())
    );
    close(root, store);
}

/// Один Sieve redirect агрегирует addresses одного mailbox, не теряя RCPT ordinals.
#[test]
fn coalesced_same_account_redirect_recovers_every_original_recipient_index() {
    let root = directory();
    let mut store = Store::open(&root).expect("open");
    store
        .put_sieve_script("bob", "forward", "redirect \"remote@peer.test\";".into())
        .expect("put script");
    store
        .activate_sieve_script("bob", Some("forward"))
        .expect("activate script");
    store
        .deliver_sieve_many(
            &["bob".into(), "bob".into()],
            "sender@peer.test",
            &["bob@litterkeep.test".into(), "bob@test".into()],
            b"Subject: coalesced recipients\r\n\r\nbody",
        )
        .expect("deliver grouped recipients");
    drop(store);

    let store = Store::open(&root).expect("reopen");
    let jobs = store.pending_outbound().expect("recovered jobs");
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs.first().map(|job| job.original_recipient_indices.clone()),
        Some(vec![0, 1])
    );
    close(root, store);
}
