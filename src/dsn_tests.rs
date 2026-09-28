//! Проверки публичного API формирования отказных уведомлений.

use super::*;
use crate::smtp::NotifyPolicy;

#[test]
fn null_reverse_path_does_not_create_a_bounce() {
    assert_eq!(
        render("", "bob@peer.test", 7, b"body", Failure::PermanentSmtp),
        None
    );
}

#[test]
fn report_preserves_original_message_bytes() {
    let original = b"Subject: source\r\n\r\npayload\x00";
    let report = render(
        "alice@litterkeep.test",
        "bob@peer.test",
        7,
        original,
        Failure::PermanentSmtp,
    )
    .expect("report");
    assert!(
        report
            .windows(original.len())
            .any(|window| window == original)
    );
    assert!(
        report
            .windows(b"report-type=delivery-status".len())
            .any(|window| { window == b"report-type=delivery-status" })
    );
    assert!(
        report
            .windows(b"Action: failed".len())
            .any(|window| window == b"Action: failed")
    );
    assert!(
        report
            .windows(b"Final-Recipient: rfc822; bob@peer.test".len())
            .any(|window| window == b"Final-Recipient: rfc822; bob@peer.test")
    );
    assert!(
        !report
            .windows(b"Final-Recipient: rfc822; <bob@peer.test>".len())
            .any(|window| window == b"Final-Recipient: rfc822; <bob@peer.test>")
    );
}

#[test]
fn boundary_moves_away_from_source_delimiter() {
    let source = b"--litterkeep-dsn-7-0\r\nbody";
    let report = render(
        "alice@litterkeep.test",
        "bob@peer.test",
        7,
        source,
        Failure::RetryExhausted,
    )
    .expect("report");
    assert!(
        report
            .windows(b"litterkeep-dsn-7-1".len())
            .any(|window| { window == b"litterkeep-dsn-7-1" })
    );
}

#[test]
fn route_policy_uses_permanent_security_status() {
    let report = render(
        "alice@litterkeep.test",
        "blocked@example.test",
        9,
        b"Subject: blocked\r\n\r\nbody",
        Failure::RoutePolicy,
    )
    .expect("report");
    assert!(
        report
            .windows(b"Status: 5.7.1".len())
            .any(|window| window == b"Status: 5.7.1")
    );
}

#[test]
fn absent_notify_policy_requests_failure_report() {
    assert!(failure_requested(None));
    let metadata = DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: None,
        origin_recipients: Vec::new(),
    };
    assert!(failure_requested(Some(&metadata)));
}

#[test]
fn notify_policy_suppresses_failure_unless_failure_is_requested() {
    assert!(!failure_requested(Some(&metadata(NotifyPolicy::Never))));
    assert!(!failure_requested(Some(&metadata(conditions(&[
        NotifyCondition::Success
    ])))));
    assert!(!failure_requested(Some(&metadata(conditions(&[
        NotifyCondition::Delay
    ])))));
    assert!(failure_requested(Some(&metadata(conditions(&[
        NotifyCondition::Failure
    ])))));
    assert!(failure_requested(Some(&metadata(conditions(&[
        NotifyCondition::Success,
        NotifyCondition::Failure,
    ])))));
}

fn metadata(notify: NotifyPolicy) -> DsnDeliveryMetadata {
    DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: Some(crate::smtp::RecipientDsn {
            notify: Some(notify),
            original_recipient: None,
        }),
        origin_recipients: Vec::new(),
    }
}

fn conditions(values: &[NotifyCondition]) -> NotifyPolicy {
    NotifyPolicy::Conditions(values.iter().copied().collect())
}
