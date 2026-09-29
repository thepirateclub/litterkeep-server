//! Формирование RFC 3464 Delivery Status Notification без доступа к Store.
//!
//! Этот модуль получает только уже принятый envelope sender, окончательный
//! результат delivery и immutable исходное сообщение. Он не открывает сокет,
//! не пишет файл и не ставит новое outbound job, поэтому пустой reverse-path
//! гарантированно не образует DSN-цикл.

use crate::{queue::DsnDeliveryMetadata, smtp::NotifyCondition};

#[path = "dsn_policy.rs"]
mod policy;
#[path = "dsn_report.rs"]
mod report;
pub(crate) use policy::notification_allowed_for_recipient;
pub use policy::{failure_requested, notification_allowed, success_requested};

/// Окончательная причина, показываемая локальному отправителю.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Failure {
    PermanentSmtp,
    RoutePolicy,
    RetryExhausted,
}

/// Тип сохраняемого уведомления, независимый от конкретной причины отказа.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Notice {
    Failure(Failure),
    Relayed,
}

impl Notice {
    /// Стабильный токен для журналов очереди и идемпотентности.
    pub(crate) fn journal_tag(self) -> &'static str {
        match self {
            Self::Failure(failure) => failure.journal_tag(),
            Self::Relayed => "relayed",
        }
    }

    /// Читает новый формат и сохранённые ранее failure-only записи.
    pub(crate) fn from_journal_tag(value: &str) -> Option<Self> {
        match Failure::from_journal_tag(value) {
            Some(failure) => Some(Self::Failure(failure)),
            None => match value {
                "relayed" => Some(Self::Relayed),
                _ => None,
            },
        }
    }
}

impl Failure {
    /// Стабильный компактный тег для queue journal и индекса идемпотентности.
    pub(crate) fn journal_tag(self) -> &'static str {
        match self {
            Self::PermanentSmtp => "permanent-smtp",
            Self::RoutePolicy => "route-policy",
            Self::RetryExhausted => "retry-exhausted",
        }
    }

    /// Разбирает только известные варианты причины из durable журнала.
    pub(crate) fn from_journal_tag(value: &str) -> Option<Self> {
        match value {
            "permanent-smtp" => Some(Self::PermanentSmtp),
            "route-policy" => Some(Self::RoutePolicy),
            "retry-exhausted" => Some(Self::RetryExhausted),
            _ => None,
        }
    }
}

/// Полный неизменяемый контекст сериализации одного DSN notice.
pub struct ReportRequest<'a> {
    /// Исходный reverse-path, которому принадлежит DSN.
    pub reverse_path: &'a str,
    /// Окончательный адрес получателя после маршрутизации.
    pub recipient: &'a str,
    /// Идентификатор исходной транзакции для стабильного MIME boundary.
    pub transaction_id: u64,
    /// Байты принятого исходного письма.
    pub original: &'a [u8],
    /// Типизированный исход delivery, а не произвольная строка отчёта.
    pub notice: Notice,
    /// Сохранённые MAIL/RCPT параметры RFC 3461, если они известны.
    pub metadata: Option<&'a DsnDeliveryMetadata>,
    /// Последний полученный SMTP diagnostic, если отказ ответил удалённый peer.
    pub diagnostic: Option<&'a crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
}

/// Outcome, сериализуемый в per-recipient блок RFC 3464.
#[derive(Clone, Copy)]
pub(crate) enum ReportOutcome {
    /// Окончательное отклонение или отказ от доставки.
    Failure(Failure),
    /// Локальное письмо уже стало доступно клиенту получателя.
    Delivered,
    /// Успешное принятие сообщения следующей SMTP-системой.
    Relayed,
}

/// Решает, разрешает ли сохранённая RCPT policy failure report.
///
/// RFC 3461 задаёт default FAILURE при отсутствии NOTIFY; явные условия без
/// FAILURE, включая SUCCESS и DELAY, запрещают failure report, как и NEVER.
/// Строит multipart/report для одного окончательно недоставленного recipient.
///
/// Пустой reverse-path обозначает DSN или auto-reply. Для него RFC-профиль
/// запрещает новый DSN, поэтому результат отсутствует до выделения памяти.
pub fn render(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    failure: Failure,
) -> Option<Vec<u8>> {
    render_with_metadata(
        reverse_path,
        recipient,
        transaction_id,
        original,
        failure,
        None,
    )
}

/// Формирует отчёт, сохраняя исходные параметры RFC 3461 конкретного RCPT.
pub fn render_with_metadata(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    failure: Failure,
    metadata: Option<&DsnDeliveryMetadata>,
) -> Option<Vec<u8>> {
    render_with_diagnostic(
        reverse_path,
        recipient,
        transaction_id,
        original,
        failure,
        metadata,
        None,
    )
}

/// Формирует отчёт с SMTP diagnostic, сохранённым исходящим transport owner-ом.
pub(crate) fn render_with_diagnostic(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    failure: Failure,
    metadata: Option<&DsnDeliveryMetadata>,
    diagnostic: Option<&crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
) -> Option<Vec<u8>> {
    match (reverse_path.is_empty(), has_failure_recipient(metadata)) {
        (true, _) | (false, false) => None,
        (false, true) => Some(report::render_with_diagnostic(
            reverse_path,
            recipient,
            transaction_id,
            original,
            ReportOutcome::Failure(failure),
            metadata,
            diagnostic,
        )),
    }
}

/// Формирует сохранённое failure либо relay уведомление с одной общей policy.
pub(crate) fn render_notice_with_diagnostic(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    notice: Notice,
    metadata: Option<&DsnDeliveryMetadata>,
    diagnostic: Option<&crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
) -> Option<Vec<u8>> {
    let requested = match notice {
        Notice::Failure(_) => failure_requested(metadata),
        Notice::Relayed => success_requested(metadata),
    };
    match (reverse_path.is_empty(), requested) {
        (false, true) => {
            let outcome = match notice {
                Notice::Failure(failure) => ReportOutcome::Failure(failure),
                Notice::Relayed => ReportOutcome::Relayed,
            };
            Some(report::render_with_diagnostic(
                reverse_path,
                recipient,
                transaction_id,
                original,
                outcome,
                metadata,
                diagnostic,
            ))
        }
        (true, _) | (false, false) => None,
    }
}

/// Проверяет, остался ли хотя бы один RCPT, имеющий право получить failure DSN.
fn has_failure_recipient(metadata: Option<&DsnDeliveryMetadata>) -> bool {
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

/// Формирует positive DSN только для явно запрошенного успешного результата.
///
/// Вызывающий код обязан вызывать функцию только после durable commit. Позитивный
/// отчёт требует `NOTIFY=SUCCESS` для конкретного RCPT и возвращает лишь
/// заголовки исходного письма независимо от RET.
pub fn render_delivery_success(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    metadata: Option<&DsnDeliveryMetadata>,
) -> Option<Vec<u8>> {
    match (
        reverse_path.is_empty(),
        notification_allowed(metadata, NotifyCondition::Success),
    ) {
        (false, true) => Some(report::render(
            reverse_path,
            recipient,
            transaction_id,
            original,
            ReportOutcome::Delivered,
            metadata,
        )),
        (true, _) | (false, false) => None,
    }
}

/// Строит отчёт из агрегированного контекста без потери RCPT-корреляции.
pub fn render_request(request: ReportRequest<'_>) -> Option<Vec<u8>> {
    render_notice_with_diagnostic(
        request.reverse_path,
        request.recipient,
        request.transaction_id,
        request.original,
        request.notice,
        request.metadata,
        request.diagnostic,
    )
}

#[cfg(test)]
#[path = "dsn_relay_tests.rs"]
mod relay_tests;
#[cfg(test)]
#[path = "dsn_tests.rs"]
mod tests;
