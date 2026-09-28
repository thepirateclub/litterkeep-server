//! Проверки идемпотентной публикации и восстановления delivered DSN.

use super::{AppendStage, Store, StoreFailpoint};
use crate::{
    dsn,
    queue::{DsnDeliveryMetadata, DsnSuccessReportKey},
    smtp::{DsnEnvelope, MailDsn, NotifyCondition, NotifyPolicy, RcptTo, RecipientDsn},
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "litterkeep-dsn-success-report-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn envelope() -> DsnEnvelope {
    DsnEnvelope {
        reverse_path: "alice@test".into(),
        mail: MailDsn::default(),
        recipients: vec![RcptTo {
            address: "bob@test".into(),
            dsn: RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Success].into_iter().collect(),
                )),
                original_recipient: None,
            },
        }],
    }
}

#[test]
fn committed_success_report_acknowledges_intent_once_across_reopen() {
    let directory = root();
    let source = b"Subject: receipt\r\n\r\noriginal body";
    let request = envelope();
    let source_transaction_id = {
        let mut store = Store::open(&directory).expect("open Store");
        store
            .deliver_sieve_many_with_dsn(
                &["bob".into()],
                "alice@test",
                &["bob@test".into()],
                source,
                Some(&request),
            )
            .expect("commit local delivery")
    };
    let key = DsnSuccessReportKey {
        source_transaction_id,
        recipient_index: 0,
    };
    let metadata = DsnDeliveryMetadata {
        mail: request.mail.clone(),
        recipient: request
            .recipients
            .first()
            .map(|recipient| recipient.dsn.clone()),
    };
    let report = dsn::render_delivery_success(
        &request.reverse_path,
        "bob@test",
        source_transaction_id,
        source,
        Some(&metadata),
    )
    .expect("explicitly requested delivered report");

    let mut store = Store::open(&directory).expect("open report writer");
    let report_transaction_id = store
        .append_dsn_success_report_once(
            crate::dsn_target::DsnSuccessTarget::LocalMailbox("alice".into()),
            &report,
            key,
        )
        .expect("append report");
    assert!(store.pending_dsn_success_notices().is_empty());
    drop(store);

    let mut reopened = Store::open(&directory).expect("recover report and acknowledgement");
    assert!(reopened.pending_dsn_success_notices().is_empty());
    assert_eq!(
        reopened
            .append_dsn_success_report_once(
                crate::dsn_target::DsnSuccessTarget::LocalMailbox("alice".into()),
                b"duplicate",
                key,
            )
            .expect("idempotent retry"),
        report_transaction_id
    );
    let messages = reopened.messages("alice").expect("sender mailbox");
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(
        messages.first().map(|message| message.bytes.as_slice()),
        Some(report.as_slice())
    );
    drop(reopened);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn report_without_a_committed_source_intent_is_rejected() {
    let directory = root();
    let mut store = Store::open(&directory).expect("open Store");
    let result = store.append_dsn_success_report_once(
        crate::dsn_target::DsnSuccessTarget::LocalMailbox("alice".into()),
        b"not a report",
        DsnSuccessReportKey {
            source_transaction_id: 99,
            recipient_index: 0,
        },
    );
    assert!(result.is_err());
    drop(store);
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn crash_before_report_commit_keeps_source_intent_retryable() {
    let directory = root();
    let request = envelope();
    let source_transaction_id = {
        let mut store = Store::open(&directory).expect("open Store");
        store
            .deliver_sieve_many_with_dsn(
                &["bob".into()],
                "alice@test",
                &["bob@test".into()],
                b"Subject: retry\r\n\r\nsource body",
                Some(&request),
            )
            .expect("commit source delivery")
    };
    let key = DsnSuccessReportKey {
        source_transaction_id,
        recipient_index: 0,
    };
    {
        let mut store = Store::open(&directory).expect("open report writer");
        store.set_failpoint(StoreFailpoint::Stage(AppendStage::DsnSuccessReportWritten));
        assert!(
            store
                .append_dsn_success_report_once(
                    crate::dsn_target::DsnSuccessTarget::LocalMailbox("alice".into()),
                    b"interrupted report",
                    key,
                )
                .is_err()
        );
    }
    let mut reopened = Store::open(&directory).expect("recover before report commit");
    assert_eq!(reopened.pending_dsn_success_notices().len(), 1);
    assert!(
        reopened
            .messages("alice")
            .expect("alice mailbox")
            .is_empty()
    );
    let report_transaction_id = reopened
        .append_dsn_success_report_once(
            crate::dsn_target::DsnSuccessTarget::LocalMailbox("alice".into()),
            b"retry report",
            key,
        )
        .expect("retry uncommitted report");
    assert!(reopened.pending_dsn_success_notices().is_empty());
    assert_eq!(
        reopened
            .dsn_success_report_transaction(&key)
            .expect("committed report index"),
        report_transaction_id
    );
    drop(reopened);
    std::fs::remove_dir_all(directory).expect("cleanup");
}
