//! Проверяет atomic visibility SMTP DSN envelope рядом с Store commit log.

use super::*;
use crate::smtp::{
    DsnEnvelope, MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient,
    RcptTo, RecipientDsn, ReturnContent,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn root() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("litterkeep-dsn-store-{timestamp}"))
}

fn envelope() -> DsnEnvelope {
    DsnEnvelope {
        reverse_path: "alice@test".into(),
        mail: MailDsn {
            return_content: Some(ReturnContent::Headers),
            envelope_id: Some(b"transaction-9".to_vec()),
        },
        recipients: vec![RcptTo {
            address: "bob@test".into(),
            dsn: RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Success].into_iter().collect(),
                )),
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Utf8,
                    address: "bob@例.test".as_bytes().to_vec(),
                }),
            },
        }],
    }
}

/// Metadata становится видимой только после общей append transaction commit.
#[test]
fn committed_envelope_survives_store_reopen() {
    let directory = root();
    let expected = envelope();
    let transaction_id = {
        let mut store = Store::open(&directory).expect("open store");
        let transaction_id = store
            .append_many_with_sieve(&["bob".into()], b"message", &[], &[], Some(&expected), &[])
            .expect("atomic message append");
        assert_eq!(
            store.smtp_dsn_envelopes.get(&transaction_id),
            Some(&expected)
        );
        transaction_id
    };
    let store = Store::open(&directory).expect("reopen committed store");
    assert_eq!(
        store.smtp_dsn_envelopes.get(&transaction_id),
        Some(&expected)
    );
    drop(store);
    let _ = fs::remove_dir_all(directory);
}

/// Scheduler получает RCPT policy, восстановленную через transaction и ordinal.
#[test]
fn outbound_job_recovers_its_recipient_policy_after_store_reopen() {
    let directory = root();
    let expected = envelope();
    let expected_recipient = expected
        .recipients
        .first()
        .map(|recipient| recipient.dsn.clone())
        .expect("one recipient policy");
    {
        let mut store = Store::open(&directory).expect("open store");
        store
            .append_many_with_sieve(
                &["bob".into()],
                b"message",
                &[("alice@test".into(), "forward@peer.test".into())],
                &[],
                Some(&expected),
                &[vec![0]],
            )
            .expect("commit queued delivery");
    }
    let store = Store::open(&directory).expect("reopen store");
    let delivery = store
        .pending_outbound()
        .expect("pending queue")
        .into_iter()
        .next()
        .expect("one delivery");
    assert_eq!(delivery.original_recipient_indices, vec![0]);
    assert_eq!(
        delivery.dsn_metadata,
        Some(Box::new(crate::queue::DsnDeliveryMetadata {
            mail: expected.mail,
            recipient: Some(expected_recipient.clone()),
            origin_recipients: vec![Some(expected_recipient.clone())],
        }))
    );
    drop(store);
    let _ = fs::remove_dir_all(directory);
}

/// Complete DSN frame без global commit игнорируется после аварийного reopen.
#[test]
fn uncommitted_envelope_is_invisible_after_reopen() {
    let directory = root();
    {
        let mut store = Store::open(&directory).expect("open store");
        store.set_failpoint(StoreFailpoint::Stage(AppendStage::EnvelopeWritten));
        assert!(
            store
                .append_many_with_sieve(
                    &["bob".into()],
                    b"message",
                    &[],
                    &[],
                    Some(&envelope()),
                    &[]
                )
                .is_err()
        );
    }
    let store = Store::open(&directory).expect("recover store without commit");
    assert!(store.smtp_dsn_envelopes.is_empty());
    drop(store);
    let _ = fs::remove_dir_all(directory);
}

/// Перезапуск не трактует старый пустой sidecar как нулевое metadata.
#[test]
fn old_store_without_dsn_sidecar_still_opens() {
    let directory = root();
    let store = Store::open(&directory).expect("create pre-extension store");
    drop(store);
    assert!(!directory.join("journal/smtp-dsn.log").exists());
    let store = Store::open(&directory).expect("open store with absent sidecar");
    assert!(store.smtp_dsn_envelopes.is_empty());
    drop(store);
    let _ = fs::remove_dir_all(directory);
}
