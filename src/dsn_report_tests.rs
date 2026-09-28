//! Проверки сериализованного RFC 3464 delivery-status MIME report.

use super::*;
use crate::dsn::render_delivery_success;
use crate::smtp::{MailDsn, OriginalRecipient, RecipientDsn};

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
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn {
            return_content: Some(ReturnContent::Full),
            ..MailDsn::default()
        },
        recipient: None,
    };
    let report =
        render_delivery_success("alice@litterkeep.test", "bob@peer.test", 22, source, None)
            .expect("non-empty reverse path produces a delivered report");
    assert!(contains(&report, b"Action: delivered\r\nStatus: 2.0.0\r\n"));
    assert!(contains(&report, b"Delivery Status Notification (Success)"));
    assert!(contains(
        &report,
        b"Content-Type: text/rfc822-headers\r\n\r\nSubject: receipt\r\nX-Trace: abc\r\n\r\n"
    ));
    assert!(!contains(&report, b"secret attachment bytes"));
    let report_with_full_return = render_delivery_success(
        "alice@litterkeep.test",
        "bob@peer.test",
        23,
        source,
        Some(&metadata),
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
    assert!(render_delivery_success("", "bob@peer.test", 24, b"body", None).is_none());
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

fn headers_metadata() -> DsnDeliveryMetadata {
    DsnDeliveryMetadata {
        mail: MailDsn {
            return_content: Some(ReturnContent::Headers),
            ..MailDsn::default()
        },
        recipient: None,
    }
}
