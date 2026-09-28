//! Проверки recipient-scoped success intents и их commit-linked recovery.

use super::{AppendStage, Store, StoreFailpoint};
use crate::{
    queue::DsnSuccessKey,
    smtp::{DsnEnvelope, MailDsn, NotifyCondition, NotifyPolicy, RcptTo, RecipientDsn},
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-dsn-success-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn recipient(address: &str, notify: NotifyPolicy) -> RcptTo {
    RcptTo {
        address: address.to_owned(),
        dsn: RecipientDsn {
            notify: Some(notify),
            original_recipient: None,
        },
    }
}

fn envelope(recipients: Vec<RcptTo>) -> DsnEnvelope {
    DsnEnvelope {
        reverse_path: "alice@test".into(),
        mail: MailDsn::default(),
        recipients,
    }
}

fn success_policy() -> NotifyPolicy {
    NotifyPolicy::Conditions([NotifyCondition::Success].into_iter().collect())
}

#[test]
fn committed_local_success_intent_survives_store_reopen() {
    let directory = root();
    let request = envelope(vec![recipient("bob@test", success_policy())]);
    let transaction_id = {
        let mut store = Store::open(&directory).expect("open store");
        store
            .deliver_sieve_many_with_dsn(
                &["bob".into()],
                "alice@test",
                &["bob@test".into()],
                b"Subject: receipt\r\n\r\nbody",
                Some(&request),
            )
            .expect("commit local delivery")
    };
    let store = Store::open(&directory).expect("reopen committed store");
    assert_eq!(
        store.pending_dsn_success_notices(),
        vec![DsnSuccessKey {
            transaction_id,
            recipient_index: 0,
        }]
    );
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn bounded_success_notice_pages_are_stable_across_recovery() {
    let directory = root();
    let mut expected = Vec::new();
    {
        let mut store = Store::open(&directory).expect("open store");
        let addresses = (0..7).map(|_| "bob@test".to_owned()).collect::<Vec<_>>();
        let request = envelope(
            addresses
                .iter()
                .map(|address| recipient(address, success_policy()))
                .collect(),
        );
        let transaction_id = store
            .deliver_sieve_many_with_dsn(
                &vec!["bob".to_owned(); 7],
                "alice@test",
                &addresses,
                b"Subject: page\r\n\r\nbody",
                Some(&request),
            )
            .expect("commit delivery");
        for recipient_index in 0..7 {
            expected.push(DsnSuccessKey {
                transaction_id,
                recipient_index,
            });
        }
        let first_page = store.pending_dsn_success_notices_after(None, 3);
        assert_eq!(
            first_page,
            expected.iter().take(3).copied().collect::<Vec<_>>()
        );
        let second_page = store.pending_dsn_success_notices_after(None, 6);
        assert_eq!(
            second_page,
            expected.iter().take(6).copied().collect::<Vec<_>>()
        );
        let cursor = expected.get(2).copied();
        assert_eq!(
            store.pending_dsn_success_notices_after(cursor, 3),
            expected.iter().skip(3).take(3).copied().collect::<Vec<_>>()
        );
        let final_key = expected.iter().last().copied();
        assert_eq!(
            store.pending_dsn_success_notices_after(final_key, 2),
            expected.iter().take(2).copied().collect::<Vec<_>>()
        );
    }
    let recovered = Store::open(&directory).expect("recover ordered intents");
    assert_eq!(
        recovered.pending_dsn_success_notices(),
        expected,
        "recovery preserves the same transaction/recipient order"
    );
    assert_eq!(
        recovered.pending_dsn_success_notices_after(None, 4),
        expected.iter().take(4).copied().collect::<Vec<_>>()
    );
    drop(recovered);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn duplicate_rcpt_tracks_only_each_explicit_success_request() {
    let directory = root();
    let request = envelope(vec![
        recipient("bob@test", success_policy()),
        recipient("bob@litterkeep.test", NotifyPolicy::Never),
    ]);
    let mut store = Store::open(&directory).expect("open store");
    let transaction_id = store
        .deliver_sieve_many_with_dsn(
            &["bob".into(), "bob".into()],
            "alice@test",
            &["bob@test".into(), "bob@litterkeep.test".into()],
            b"Subject: duplicate recipients\r\n\r\nbody",
            Some(&request),
        )
        .expect("commit local delivery");
    assert_eq!(
        store.pending_dsn_success_notices(),
        vec![DsnSuccessKey {
            transaction_id,
            recipient_index: 0,
        }]
    );
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

/// Один local Sieve plan создаёт отдельный intent для каждого RCPT SUCCESS.
#[test]
fn duplicate_rcpt_success_requests_keep_independent_recipient_intents() {
    let directory = root();
    let addresses = vec!["bob@test".to_owned(), "bob@litterkeep.test".to_owned()];
    let request = envelope(
        addresses
            .iter()
            .map(|address| recipient(address, success_policy()))
            .collect(),
    );
    let transaction_id = {
        let mut store = Store::open(&directory).expect("open store");
        let transaction_id = store
            .deliver_sieve_many_with_dsn(
                &["bob".into(), "bob".into()],
                "alice@test",
                &addresses,
                b"Subject: two recipient reports\r\n\r\nbody",
                Some(&request),
            )
            .expect("commit one local delivery");
        assert_eq!(store.messages("bob").expect("mailbox").len(), 1);
        transaction_id
    };
    let recovered = Store::open(&directory).expect("reopen store");
    assert_eq!(
        recovered.pending_dsn_success_notices(),
        vec![
            DsnSuccessKey {
                transaction_id,
                recipient_index: 0,
            },
            DsnSuccessKey {
                transaction_id,
                recipient_index: 1,
            },
        ]
    );
    assert_eq!(recovered.messages("bob").expect("mailbox").len(), 1);
    drop(recovered);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn failure_only_policy_does_not_create_success_intent() {
    let directory = root();
    let request = envelope(vec![recipient(
        "bob@test",
        NotifyPolicy::Conditions([NotifyCondition::Failure].into_iter().collect()),
    )]);
    let mut store = Store::open(&directory).expect("open store");
    store
        .deliver_sieve_many_with_dsn(
            &["bob".into()],
            "alice@test",
            &["bob@test".into()],
            b"Subject: failure only\r\n\r\nbody",
            Some(&request),
        )
        .expect("commit local delivery");
    assert!(store.pending_dsn_success_notices().is_empty());
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn uncommitted_success_intent_is_hidden_after_recovery() {
    let directory = root();
    {
        let request = envelope(vec![recipient("bob@test", success_policy())]);
        let mut store = Store::open(&directory).expect("open store");
        store.set_failpoint(StoreFailpoint::Stage(AppendStage::DsnSuccessWritten));
        assert!(
            store
                .deliver_sieve_many_with_dsn(
                    &["bob".into()],
                    "alice@test",
                    &["bob@test".into()],
                    b"Subject: interrupted\r\n\r\nbody",
                    Some(&request),
                )
                .is_err()
        );
    }
    let store = Store::open(&directory).expect("recover uncommitted intent");
    assert!(store.pending_dsn_success_notices().is_empty());
    assert!(store.smtp_dsn_envelopes.is_empty());
    assert!(store.messages("bob").expect("mailbox").is_empty());
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn discard_plan_does_not_create_local_success_intent() {
    let directory = root();
    let request = envelope(vec![recipient("bob@test", success_policy())]);
    let mut store = Store::open(&directory).expect("open store");
    store
        .put_sieve_script("bob", "discard", "discard; stop;".into())
        .expect("put script");
    store
        .activate_sieve_script("bob", Some("discard"))
        .expect("activate script");
    store
        .deliver_sieve_many_with_dsn(
            &["bob".into()],
            "alice@test",
            &["bob@test".into()],
            b"Subject: discarded\r\n\r\nbody",
            Some(&request),
        )
        .expect("commit discard transaction");
    assert!(store.pending_dsn_success_notices().is_empty());
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}
