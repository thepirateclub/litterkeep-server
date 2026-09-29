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
