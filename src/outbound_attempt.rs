//! Результат одного завершённого исходящего SMTP transaction.

use crate::{outbound::DeliveryOutcome, smtp_reply_diagnostic::SmtpReplyDiagnostic};

/// Семантический outcome и сохранённый ответ peer, если отказ был SMTP.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeliveryAttempt {
    pub outcome: DeliveryOutcome,
    pub diagnostic: Option<SmtpReplyDiagnostic>,
}
