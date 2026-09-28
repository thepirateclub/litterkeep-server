//! RFC 3461 outcome vectors без SMTP wire capability.

use super::notification_allowed;
use crate::{
    dsn,
    queue::DsnDeliveryMetadata,
    smtp::{NotifyCondition, NotifyPolicy, RecipientDsn},
};

#[test]
fn default_policy_requires_failure_but_does_not_request_success() {
    assert!(notification_allowed(None, NotifyCondition::Failure));
    assert!(!notification_allowed(None, NotifyCondition::Success));
    assert!(notification_allowed(None, NotifyCondition::Delay));
    assert!(dsn::failure_requested(None));
}

#[test]
fn explicit_condition_list_only_allows_listed_outcomes() {
    let metadata = metadata(NotifyPolicy::Conditions(
        [NotifyCondition::Success, NotifyCondition::Delay]
            .into_iter()
            .collect(),
    ));
    assert!(notification_allowed(
        Some(&metadata),
        NotifyCondition::Success
    ));
    assert!(!notification_allowed(
        Some(&metadata),
        NotifyCondition::Failure
    ));
    assert!(notification_allowed(
        Some(&metadata),
        NotifyCondition::Delay
    ));
}

#[test]
fn never_suppresses_every_outcome() {
    let metadata = metadata(NotifyPolicy::Never);
    for condition in [
        NotifyCondition::Success,
        NotifyCondition::Failure,
        NotifyCondition::Delay,
    ] {
        assert!(!notification_allowed(Some(&metadata), condition));
    }
}

#[test]
fn grouped_aliases_request_failure_when_any_origin_requests_it() {
    let mut metadata = metadata(NotifyPolicy::Never);
    metadata.origin_recipients = vec![
        Some(RecipientDsn {
            notify: Some(NotifyPolicy::Never),
            original_recipient: None,
        }),
        Some(RecipientDsn {
            notify: Some(NotifyPolicy::Conditions(
                [NotifyCondition::Failure].into_iter().collect(),
            )),
            original_recipient: None,
        }),
    ];
    assert!(dsn::failure_requested(Some(&metadata)));

    metadata.origin_recipients.pop();
    assert!(!dsn::failure_requested(Some(&metadata)));
}

fn metadata(notify: NotifyPolicy) -> DsnDeliveryMetadata {
    DsnDeliveryMetadata {
        mail: Default::default(),
        recipient: Some(RecipientDsn {
            notify: Some(notify),
            original_recipient: None,
        }),
        origin_recipients: Vec::new(),
    }
}
