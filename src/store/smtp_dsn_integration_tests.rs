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
            .append_many_with_sieve(&["bob".into()], b"message", &[], &[], Some(&expected))
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

/// Complete DSN frame без global commit игнорируется после аварийного reopen.
#[test]
fn uncommitted_envelope_is_invisible_after_reopen() {
    let directory = root();
    {
        let mut store = Store::open(&directory).expect("open store");
        store.set_failpoint(StoreFailpoint::Stage(AppendStage::EnvelopeWritten));
        assert!(
            store
                .append_many_with_sieve(&["bob".into()], b"message", &[], &[], Some(&envelope()))
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
