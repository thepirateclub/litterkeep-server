//! Проверяет несколько eligible исходных RCPT в одном DSN report.
use super::*;

/// Одна ошибка redirect содержит отдельный RFC 3464 block для каждого origin.
#[test]
fn failure_report_keeps_every_origin_recipient_group() {
    let metadata = DsnDeliveryMetadata {
        mail: MailDsn {
            envelope_id: Some(b"multi-origin-42".to_vec()),
            ..MailDsn::default()
        },
        recipient: None,
        origin_recipients: vec![
            Some(origin(b"first@alias.test")),
            Some(origin(b"second@alias.test")),
        ],
    };
    let report = render(
        "sender@origin.test",
        "forward@relay.test",
        42,
        b"Subject: grouped failure\r\n\r\nbody",
        ReportOutcome::Failure(Failure::PermanentSmtp),
        Some(&metadata),
    );

    assert_eq!(count(&report, b"Final-Recipient:"), 2);
    assert_eq!(count(&report, b"Action: failed\r\n"), 2);
    assert!(contains(
        &report,
        b"Original-Recipient: rfc822; first@alias.test\r\n"
    ));
    assert!(contains(
        &report,
        b"Original-Recipient: rfc822; second@alias.test\r\n"
    ));
    assert_eq!(
        count(&report, b"Original-Envelope-ID: multi-origin-42\r\n"),
        1
    );
}

/// Создаёт origin с явной recipient-scoped failure policy.
fn origin(address: &[u8]) -> crate::smtp::RecipientDsn {
    crate::smtp::RecipientDsn {
        notify: Some(crate::smtp::NotifyPolicy::Conditions(
            [crate::smtp::NotifyCondition::Failure]
                .into_iter()
                .collect(),
        )),
        original_recipient: Some(crate::smtp::OriginalRecipient {
            address_type: crate::smtp::OriginalAddressType::Rfc822,
            address: address.to_vec(),
        }),
    }
}

/// Считает поля без разбора MIME-представления тестируемого report.
fn count(report: &[u8], field: &[u8]) -> usize {
    report
        .windows(field.len())
        .filter(|candidate| *candidate == field)
        .count()
}

/// Проверяет наличие точных исходных octets в сериализованном отчёте.
fn contains(report: &[u8], field: &[u8]) -> bool {
    count(report, field) > 0
}
