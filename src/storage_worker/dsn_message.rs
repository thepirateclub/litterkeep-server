//! Типизированные request/response контракты доставки DSN.

use std::io;

/// Запрос Store owner-у на публикацию одного DSN outcome.
#[derive(Debug)]
pub enum DsnRequest {
    /// Финальная ошибка исходящей SMTP-доставки.
    Failure {
        request_id: u64,
        reverse_path: String,
        recipient: String,
        transaction_id: u64,
        original: Vec<u8>,
        failure: crate::dsn::Failure,
        dsn_metadata: Option<Box<crate::queue::DsnDeliveryMetadata>>,
        diagnostic: Option<crate::smtp_reply_diagnostic::SmtpReplyDiagnostic>,
        report_key: crate::queue::DsnReportKey,
    },
    /// Локальная доставка завершилась, и требуется positive report.
    Success {
        request_id: u64,
        key: crate::queue::DsnSuccessKey,
    },
}

impl DsnRequest {
    /// Возвращает общий request ID для регистрации ожидания в reactor-е.
    pub fn request_id(&self) -> u64 {
        match self {
            Self::Failure { request_id, .. } | Self::Success { request_id, .. } => *request_id,
        }
    }
}

/// Завершение typed DSN-команды с сохранённой корреляцией.
#[derive(Debug)]
pub enum DsnResponse {
    /// Положительное подтверждение либо отсутствие локального sender account.
    Failure {
        request_id: u64,
        result: io::Result<Option<u64>>,
    },
    /// Положительное подтверждение либо отсутствие локальной цели.
    Success {
        request_id: u64,
        result: io::Result<Option<u64>>,
    },
}

impl DsnResponse {
    /// Возвращает ID исходного bounded запроса.
    pub fn request_id(&self) -> u64 {
        match self {
            Self::Failure { request_id, .. } | Self::Success { request_id, .. } => *request_id,
        }
    }
}
