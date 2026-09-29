//! Выбирает текстовые и статусные поля одного RFC 3464 отчёта.

use crate::dsn::{Failure, ReportOutcome};

/// Возвращает тему, пояснение, Action и Enhanced Status Code для outcome.
pub(super) fn report_fields(
    outcome: ReportOutcome,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match outcome {
        ReportOutcome::Delivered => (
            "Success",
            "Your message was successfully delivered to",
            "delivered",
            "2.0.0",
        ),
        ReportOutcome::Relayed => (
            "Relayed",
            "Your message was successfully relayed to",
            "relayed",
            "2.0.0",
        ),
        ReportOutcome::Delayed => (
            "Delayed",
            "Delivery is still being attempted for",
            "delayed",
            "4.0.0",
        ),
        ReportOutcome::Failure(failure) => match failure {
            Failure::PermanentSmtp => ("Failure", "Delivery failed for", "failed", "5.0.0"),
            Failure::RoutePolicy => (
                "Failure",
                "Delivery was denied by recipient policy for",
                "failed",
                "5.7.1",
            ),
            Failure::RetryExhausted => (
                "Failure",
                "Delivery retry budget was exhausted for",
                "failed",
                "4.4.0",
            ),
        },
    }
}
