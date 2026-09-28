//! Проверка соответствия typed DSN данных SMTP получателям транзакции.

use crate::smtp::DsnEnvelope;

/// Сравнивает reverse-path, число и порядок получателей без потери параметров.
pub(super) fn matches(envelope: &DsnEnvelope, reverse_path: &str, recipients: &[String]) -> bool {
    let sender_matches = envelope.reverse_path == reverse_path;
    let count_matches = envelope.recipients.len() == recipients.len();
    let recipients_match = envelope
        .recipients
        .iter()
        .zip(recipients)
        .all(|(dsn_recipient, address)| dsn_recipient.address == *address);
    matches!(
        (sender_matches, count_matches, recipients_match),
        (true, true, true)
    )
}
