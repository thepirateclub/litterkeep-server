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
pub use policy::notification_allowed;

/// Окончательная причина, показываемая локальному отправителю.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Failure {
    PermanentSmtp,
    RoutePolicy,
    RetryExhausted,
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

/// Полный неизменяемый контекст сериализации одного failure DSN.
pub struct ReportRequest<'a> {
    /// Исходный reverse-path, которому принадлежит DSN.
    pub reverse_path: &'a str,
    /// Окончательный адрес получателя после маршрутизации.
    pub recipient: &'a str,
    /// Идентификатор исходной транзакции для стабильного MIME boundary.
    pub transaction_id: u64,
    /// Байты принятого исходного письма.
    pub original: &'a [u8],
    /// Типизированная окончательная причина отказа.
    pub failure: Failure,
    /// Сохранённые MAIL/RCPT параметры RFC 3461, если они известны.
    pub metadata: Option<&'a DsnDeliveryMetadata>,
}

/// Outcome, сериализуемый в per-recipient блок RFC 3464.
#[derive(Clone, Copy)]
pub(crate) enum ReportOutcome {
    /// Окончательное отклонение или отказ от доставки.
    Failure(Failure),
    /// Локальное письмо уже стало доступно клиенту получателя.
    Delivered,
}

/// Решает, разрешает ли сохранённая RCPT policy failure report.
///
/// RFC 3461 задаёт default FAILURE при отсутствии NOTIFY; явные условия без
/// FAILURE, включая SUCCESS и DELAY, запрещают failure report, как и NEVER.
pub fn failure_requested(metadata: Option<&DsnDeliveryMetadata>) -> bool {
    notification_allowed(metadata, NotifyCondition::Failure)
}

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
    match reverse_path.is_empty() {
        true => None,
        false => Some(report::render(
            reverse_path,
            recipient,
            transaction_id,
            original,
            ReportOutcome::Failure(failure),
            metadata,
        )),
    }
}

/// Формирует positive DSN для уже опубликованного локального mailbox delivery.
///
/// Вызывающий код обязан вызывать функцию только после durable commit. Позитивный
/// отчёт всегда возвращает лишь заголовки исходного письма, независимо от RET.
pub fn render_delivery_success(
    reverse_path: &str,
    recipient: &str,
    transaction_id: u64,
    original: &[u8],
    metadata: Option<&DsnDeliveryMetadata>,
) -> Option<Vec<u8>> {
    match reverse_path.is_empty() {
        true => None,
        false => Some(report::render(
            reverse_path,
            recipient,
            transaction_id,
            original,
            ReportOutcome::Delivered,
            metadata,
        )),
    }
}

/// Строит отчёт из агрегированного контекста без потери RCPT-корреляции.
pub fn render_request(request: ReportRequest<'_>) -> Option<Vec<u8>> {
    render_with_metadata(
        request.reverse_path,
        request.recipient,
        request.transaction_id,
        request.original,
        request.failure,
        request.metadata,
    )
}

#[cfg(test)]
mod tests {
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

    /// При отсутствии запросной политики сохраняется стандартное failure DSN.
    #[test]
    fn absent_notify_policy_requests_failure_report() {
        assert!(failure_requested(None));
        let metadata = DsnDeliveryMetadata {
            mail: Default::default(),
            recipient: None,
        };
        assert!(failure_requested(Some(&metadata)));
    }

    /// Явные NEVER, SUCCESS и DELAY не разрешают failure report.
    #[test]
    fn notify_policy_suppresses_failure_unless_failure_is_requested() {
        assert!(!failure_requested(Some(&metadata(NotifyPolicy::Never))));
        assert!(!failure_requested(Some(&metadata(conditions(&[
            NotifyCondition::Success,
        ])))));
        assert!(!failure_requested(Some(&metadata(conditions(&[
            NotifyCondition::Delay,
        ])))));
        assert!(failure_requested(Some(&metadata(conditions(&[
            NotifyCondition::Failure,
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
        }
    }

    fn conditions(values: &[NotifyCondition]) -> NotifyPolicy {
        NotifyPolicy::Conditions(values.iter().copied().collect())
    }
}
