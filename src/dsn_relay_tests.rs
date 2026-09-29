//! Проверяет сериализацию отдельного RFC 3464 relay notice.

use super::{Notice, render_notice_with_diagnostic};
use crate::{
    queue::DsnDeliveryMetadata,
    smtp::{NotifyCondition, NotifyPolicy, RecipientDsn},
};

/// Relay report использует action=relayed и не возвращает тело письма.
#[test]
fn relayed_report_is_positive_and_headers_only() {
    let metadata = DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: Some(RecipientDsn {
            notify: Some(NotifyPolicy::Conditions(
                [NotifyCondition::Success].into_iter().collect(),
            )),
            original_recipient: None,
        }),
        origin_recipients: Vec::new(),
    };

    let report = render_notice_with_diagnostic(
        "alice@origin.test",
        "bob@forward.test",
        51,
        b"Subject: forwarded\r\n\r\nprivate body",
        Notice::Relayed,
        Some(&metadata),
        None,
    )
    .expect("explicitly requested relay notice");
    let report = String::from_utf8(report).expect("ASCII report");

    assert!(report.contains("Action: relayed\r\nStatus: 2.0.0\r\n"));
    assert!(report.contains("successfully relayed"));
    assert!(report.contains("Subject: forwarded"));
    assert!(!report.contains("private body"));
}

/// Групповой relay report включает только исходные адреса с NOTIFY=SUCCESS.
#[test]
fn grouped_relay_report_filters_origins_by_success_policy() {
    let metadata = DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: None,
        origin_recipients: vec![
            Some(recipient_with_notify(NotifyCondition::Success, "first")),
            Some(recipient_with_notify(NotifyCondition::Failure, "second")),
        ],
    };

    let report = render_notice_with_diagnostic(
        "alice@origin.test",
        "bob@forward.test",
        52,
        b"Subject: grouped\r\n\r\nprivate body",
        Notice::Relayed,
        Some(&metadata),
        None,
    )
    .expect("one origin requested SUCCESS");
    let report = String::from_utf8(report).expect("ASCII report");

    assert!(report.contains("first@origin.test"));
    assert!(!report.contains("second@origin.test"));
}

/// Delay report следует RFC 3464 и не содержит третью MIME часть с письмом.
#[test]
fn delayed_report_is_temporary_and_omits_original_message() {
    let metadata = DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: Some(recipient_with_notify(NotifyCondition::Delay, "alice")),
        origin_recipients: Vec::new(),
    };
    let report = render_notice_with_diagnostic(
        "alice@origin.test",
        "bob@forward.test",
        53,
        b"Subject: private subject\r\n\r\nprivate body",
        Notice::Delayed,
        Some(&metadata),
        None,
    )
    .expect("explicit NOTIFY=DELAY");
    let report = String::from_utf8(report).expect("ASCII report");

    assert!(report.contains("Action: delayed\r\nStatus: 4.0.0\r\n"));
    assert!(report.contains("Delivery is still being attempted"));
    assert!(!report.contains("Content-Type: message/delivery-status\r\n\r\n--"));
    assert!(!report.contains("private subject"));
    assert!(!report.contains("private body"));
}

/// Строит origin policy с уникальным ORCPT для проверки группировки.
fn recipient_with_notify(condition: NotifyCondition, local: &str) -> RecipientDsn {
    RecipientDsn {
        notify: Some(NotifyPolicy::Conditions([condition].into_iter().collect())),
        original_recipient: Some(crate::smtp::OriginalRecipient {
            address_type: crate::smtp::OriginalAddressType::Rfc822,
            address: format!("{local}@origin.test").into_bytes(),
        }),
    }
}
