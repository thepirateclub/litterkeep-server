//! Выбор исходных RCPT, которым локальный delivery plan дал SUCCESS.

use crate::{queue::DsnDeliveryMetadata, smtp::NotifyCondition};

/// Возвращает только explicit SUCCESS получателей данного локального account.
pub(super) fn requested_for_account(
    account: &str,
    accounts: &[String],
    envelope_recipients: &[String],
    smtp_dsn: Option<&crate::smtp::DsnEnvelope>,
) -> Vec<usize> {
    let Some(envelope) = smtp_dsn else {
        return Vec::new();
    };
    if envelope.reverse_path.is_empty() {
        return Vec::new();
    }
    accounts
        .iter()
        .zip(envelope_recipients)
        .enumerate()
        .filter_map(|(index, (recipient_account, _))| {
            let same_account = recipient_account == account;
            let requests_success = envelope.recipients.get(index).is_some_and(|recipient| {
                let metadata = DsnDeliveryMetadata {
                    mail: envelope.mail.clone(),
                    recipient: Some(recipient.dsn.clone()),
                };
                crate::dsn::notification_allowed(Some(&metadata), NotifyCondition::Success)
            });
            match (same_account, requests_success) {
                (true, true) => Some(index),
                (false, _) | (true, false) => None,
            }
        })
        .collect()
}

/// Различает план с локальной mailbox публикацией от чистого discard/redirect.
pub(super) fn has_mailbox_target(keep: bool, folder_count: usize) -> bool {
    match (keep, folder_count) {
        (true, _) | (false, 1..) => true,
        (false, 0) => false,
    }
}
