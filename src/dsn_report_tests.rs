//! Проверки сериализованного RFC 3464 delivery-status MIME report.

use super::*;
use crate::dsn::render_delivery_success;
use crate::smtp::{MailDsn, NotifyCondition, NotifyPolicy, OriginalRecipient, RecipientDsn};

/// Проверяет перенос sidecar-параметров MAIL и RCPT в отдельные DSN-поля.
#[test]
fn report_contains_original_envelope_and_recipient() {
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn {
            envelope_id: Some(b"batch 42".to_vec()),
            ..MailDsn::default()
        },
        recipient: Some(RecipientDsn {
            original_recipient: Some(OriginalRecipient {
                address_type: OriginalAddressType::Rfc822,
                address: b"alias@peer.test".to_vec(),
            }),
            ..RecipientDsn::default()
        }),
        origin_recipients: Vec::new(),
    };
    let report = render(
        "alice@litterkeep.test",
        "final@peer.test",
        17,
        b"Subject: original\r\n\r\nbody",
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );
    assert!(contains(&report, b"Original-Envelope-ID: batch 42\r\n"));
    assert!(contains(
        &report,
        b"Original-Recipient: rfc822; alias@peer.test\r\n"
    ));
    assert!(contains(
        &report,
        b"Original-Envelope-ID: batch 42\r\n\r\nOriginal-Recipient: rfc822; alias@peer.test\r\nFinal-Recipient: rfc822; final@peer.test"
    ));
}

/// Проверяет, что RET=HDRS исключает исходное тело и при malformed source.
#[test]
fn headers_return_policy_excludes_the_message_body() {
    let metadata = headers_metadata();
    let source = b"Subject: secret subject\r\nX-Trace: one\r\n\r\nsecret body bytes";
    let report = render(
        "alice@litterkeep.test",
        "bob@peer.test",
        18,
        source,
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );
    assert!(contains(
        &report,
        b"Subject: secret subject\r\nX-Trace: one\r\n\r\n"
    ));
    assert!(contains(&report, b"Content-Type: text/rfc822-headers"));
    assert!(!contains(&report, b"secret body bytes"));
    let malformed = render(
        "alice@litterkeep.test",
        "bob@peer.test",
        21,
        b"unseparated secret bytes",
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );
    assert!(!contains(&malformed, b"unseparated secret bytes"));
}

/// RFC 6533 использует global-headers для исходных заголовков с Unicode.
#[test]
fn international_headers_use_the_global_headers_media_type() {
    let report = render(
        "alice@litterkeep.test",
        "bob@peer.test",
        20,
        "Subject: Привет\r\n\r\nbody".as_bytes(),
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&headers_metadata()),
    );
    assert!(contains(&report, b"Content-Type: message/global-headers"));
    assert!(!contains(&report, b"body"));
}

/// Проверяет корректный Action/Status и RFC-ограничение позитивного DSN.
#[test]
fn delivered_report_uses_success_fields_and_returns_headers_only() {
    let source = b"Subject: receipt\r\nX-Trace: abc\r\n\r\nsecret attachment bytes";
    let requested = success_metadata(Some(ReturnContent::Headers));
    let report = render_delivery_success(
        "alice@litterkeep.test",
        "bob@peer.test",
        22,
        source,
        Some(&requested),
    )
    .expect("requested delivery success produces a report");
    assert!(contains(&report, b"Action: delivered\r\nStatus: 2.0.0\r\n"));
    assert!(contains(&report, b"Delivery Status Notification (Success)"));
    assert!(contains(
        &report,
        b"Content-Type: text/rfc822-headers\r\n\r\nSubject: receipt\r\nX-Trace: abc\r\n\r\n"
    ));
    assert!(!contains(&report, b"secret attachment bytes"));
    let full_return = success_metadata(Some(ReturnContent::Full));
    let report_with_full_return = render_delivery_success(
        "alice@litterkeep.test",
        "bob@peer.test",
        23,
        source,
        Some(&full_return),
    )
    .expect("full RET still produces a delivered report");
    assert!(!contains(
        &report_with_full_return,
        b"secret attachment bytes"
    ));
    assert!(contains(
        &report_with_full_return,
        b"Content-Type: text/rfc822-headers"
    ));
}

/// Запрещает DSN-цикл при пустом исходном reverse-path.
#[test]
fn delivered_report_is_suppressed_for_null_reverse_path() {
    let metadata = success_metadata(None);
    assert!(render_delivery_success("", "bob@peer.test", 24, b"body", Some(&metadata)).is_none());
}

/// Не выдаёт success DSN без явного запроса текущего RCPT.
#[test]
fn delivered_report_requires_explicit_success_policy() {
    let absent = DsnDeliveryMetadata {
        mail: MailDsn::default(),
        recipient: None,
        origin_recipients: Vec::new(),
    };
    let failure_only = DsnDeliveryMetadata {
        mail: MailDsn::default(),
        recipient: Some(RecipientDsn {
            notify: Some(NotifyPolicy::Conditions(
                [NotifyCondition::Failure].into_iter().collect(),
            )),
            original_recipient: None,
        }),
        origin_recipients: Vec::new(),
    };
    let never = DsnDeliveryMetadata {
        mail: MailDsn::default(),
        recipient: Some(RecipientDsn {
            notify: Some(NotifyPolicy::Never),
            original_recipient: None,
        }),
        origin_recipients: Vec::new(),
    };
    for metadata in [absent, failure_only, never] {
        assert!(
            render_delivery_success(
                "alice@litterkeep.test",
                "bob@peer.test",
                25,
                b"Subject: test\r\n\r\nbody",
                Some(&metadata),
            )
            .is_none()
        );
    }
}

/// RFC 6533 ORCPT сохраняет зарегистрированный тип utf-8.
#[test]
fn utf8_original_recipient_keeps_its_registered_address_type() {
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn::default(),
        recipient: Some(RecipientDsn {
            original_recipient: Some(OriginalRecipient {
                address_type: OriginalAddressType::Utf8,
                address: "用户@例.test".as_bytes().to_vec(),
            }),
            ..RecipientDsn::default()
        }),
        origin_recipients: Vec::new(),
    };
    let report = render(
        "alice@litterkeep.test",
        "user@例.test",
        19,
        b"Subject: original\r\n\r\nbody",
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );
    assert!(contains(
        &report,
        "Original-Recipient: utf-8; 用户@例.test\r\n".as_bytes()
    ));
    assert!(contains(
        &report,
        b"Content-Type: message/global-delivery-status"
    ));
    assert!(contains(
        &report,
        "Final-Recipient: utf-8; user@例.test\r\n".as_bytes()
    ));
}

/// В общем redirect DSN содержит только alias-и, запросившие FAILURE.
#[test]
fn grouped_alias_report_filters_each_origin_notify_policy() {
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn {
            envelope_id: Some(b"group-7".to_vec()),
            ..MailDsn::default()
        },
        recipient: None,
        origin_recipients: vec![
            Some(RecipientDsn {
                notify: Some(NotifyPolicy::Never),
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Rfc822,
                    address: b"silent@origin.test".to_vec(),
                }),
            }),
            Some(RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Failure].into_iter().collect(),
                )),
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Rfc822,
                    address: b"notified@origin.test".to_vec(),
                }),
            }),
        ],
    };
    let report = render(
        "alice@litterkeep.test",
        "redirect@local.test",
        27,
        b"Subject: grouped\r\n\r\nbody",
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );
    let recipient_field = b"Final-Recipient:";
    assert_eq!(
        report
            .windows(recipient_field.len())
            .filter(|value| *value == recipient_field)
            .count(),
        1
    );
    assert!(contains(&report, b"Original-Envelope-ID: group-7\r\n"));
    assert!(contains(
        &report,
        b"Original-Recipient: rfc822; notified@origin.test\r\n"
    ));
    assert!(!contains(&report, b"silent@origin.test"));
}

fn headers_metadata() -> DsnDeliveryMetadata {
    DsnDeliveryMetadata {
        mail: MailDsn {
            return_content: Some(ReturnContent::Headers),
            ..MailDsn::default()
        },
        recipient: None,
        origin_recipients: Vec::new(),
    }
}

fn success_metadata(return_content: Option<ReturnContent>) -> DsnDeliveryMetadata {
    DsnDeliveryMetadata {
        mail: MailDsn {
            return_content,
            ..MailDsn::default()
        },
        recipient: Some(RecipientDsn {
            notify: Some(NotifyPolicy::Conditions(
                [NotifyCondition::Success].into_iter().collect(),
            )),
            original_recipient: None,
        }),
        origin_recipients: Vec::new(),
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[path = "dsn_report_ret_full_tests.rs"]
mod ret_full_tests;

#[path = "dsn_report_multi_origin_tests.rs"]
mod multi_origin_tests;
