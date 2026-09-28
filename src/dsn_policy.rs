//! Выбор DSN outcome по recipient-scoped RFC 3461 NOTIFY policy.

use crate::{
    queue::DsnDeliveryMetadata,
    smtp::{NotifyCondition, NotifyPolicy},
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
    let policy = metadata
        .and_then(|value| value.recipient.as_ref())
        .and_then(|value| value.notify.as_ref());
    match policy {
        None => default_allows(condition),
        Some(NotifyPolicy::Never) => false,
        Some(NotifyPolicy::Conditions(conditions)) => conditions.contains(&condition),
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
