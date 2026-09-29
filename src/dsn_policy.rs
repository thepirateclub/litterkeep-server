//! Выбор DSN outcome по recipient-scoped RFC 3461 NOTIFY policy.

use crate::{
    queue::DsnDeliveryMetadata,
    smtp::{NotifyCondition, NotifyPolicy, RecipientDsn},
};

/// Проверяет, разрешено ли уведомление заданного outcome для одного RCPT.
///
/// Без NOTIFY отказ обязателен к уведомлению, успех не запрашивался, а delay
/// уведомление разрешено как опциональное решение MTA. Явный список условий
/// ограничивает результат только перечисленными значениями; NEVER запрещает все.
pub fn notification_allowed(
    metadata: Option<&DsnDeliveryMetadata>,
    condition: NotifyCondition,
) -> bool {
    notification_allowed_for_recipient(
        metadata.and_then(|value| value.recipient.as_ref()),
        condition,
    )
}

/// Проверяет одну recipient policy без подмены другими alias-ами.
pub(crate) fn notification_allowed_for_recipient(
    recipient: Option<&RecipientDsn>,
    condition: NotifyCondition,
) -> bool {
    let policy = recipient.and_then(|value| value.notify.as_ref());
    match policy {
        None => default_allows(condition),
        Some(NotifyPolicy::Never) => false,
        Some(NotifyPolicy::Conditions(conditions)) => conditions.contains(&condition),
    }
}

/// Разрешает failure report, если он запрошен хотя бы одним исходным RCPT.
pub fn failure_requested(metadata: Option<&DsnDeliveryMetadata>) -> bool {
    let Some(metadata) = metadata else {
        return true;
    };
    match metadata.origin_recipients.is_empty() {
        true => notification_allowed(Some(metadata), NotifyCondition::Failure),
        false => metadata.origin_recipients.iter().any(|recipient| {
            notification_allowed_for_recipient(recipient.as_ref(), NotifyCondition::Failure)
        }),
    }
}

/// Проверяет явный SUCCESS хотя бы одного исходного получателя.
pub fn success_requested(metadata: Option<&DsnDeliveryMetadata>) -> bool {
    let Some(metadata) = metadata else {
        return false;
    };
    match metadata.origin_recipients.is_empty() {
        true => notification_allowed(Some(metadata), NotifyCondition::Success),
        false => metadata.origin_recipients.iter().any(|recipient| {
            notification_allowed_for_recipient(recipient.as_ref(), NotifyCondition::Success)
        }),
    }
}

/// Проверяет, разрешена ли опциональная задержанная DSN хотя бы одному origin.
pub fn delay_requested(metadata: Option<&DsnDeliveryMetadata>) -> bool {
    let Some(metadata) = metadata else {
        return true;
    };
    match metadata.origin_recipients.is_empty() {
        true => notification_allowed(Some(metadata), NotifyCondition::Delay),
        false => metadata.origin_recipients.iter().any(|recipient| {
            notification_allowed_for_recipient(recipient.as_ref(), NotifyCondition::Delay)
        }),
    }
}

/// Применяет RFC default при отсутствии параметра NOTIFY.
fn default_allows(condition: NotifyCondition) -> bool {
    match condition {
        NotifyCondition::Failure | NotifyCondition::Delay => true,
        NotifyCondition::Success => false,
    }
}

#[cfg(test)]
#[path = "dsn_policy_tests.rs"]
mod tests;
