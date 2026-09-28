//! Типизированные данные поиска SCRAM verifier владельцем учётных записей.

use crate::sasl::scram_sha1_record::ScramSha1Record;
use std::io;

/// Запрос reactor-а на чтение verifier без передачи пароля или Store.
#[derive(Debug)]
pub struct ScramVerifierRequest {
    pub request_id: u64,
    pub account: String,
}

/// Результат чтения verifier с сохранением ошибок файлового уровня.
pub struct ScramVerifierResponse {
    pub request_id: u64,
    pub result: io::Result<Option<ScramSha1Record>>,
}

/// Запрос чтения SCRAM-SHA-256 record владельцем Accounts.
#[derive(Debug)]
pub struct ScramSha256VerifierRequest {
    pub request_id: u64,
    pub account: String,
}

/// Результат чтения SHA-256 verifier, не показывающий secret в Debug.
pub struct ScramSha256VerifierResponse {
    pub request_id: u64,
    pub result: io::Result<Option<crate::sasl::scram_sha256_record::ScramSha256Record>>,
}

/// Не включает verifier в диагностический вывод, так как это credential.
impl std::fmt::Debug for ScramVerifierResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScramVerifierResponse")
            .field("request_id", &self.request_id)
            .field("result", &"<credential omitted>")
            .finish()
    }
}

/// Исключает SHA-256 key material из отладочного вывода каналов.
impl std::fmt::Debug for ScramSha256VerifierResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScramSha256VerifierResponse")
            .field("request_id", &self.request_id)
            .field("result", &"<credential omitted>")
            .finish()
    }
}

/// Загружает verifier только через существующего владельца Accounts.
pub(super) fn load(
    accounts: &crate::accounts::Accounts,
    request: ScramVerifierRequest,
) -> ScramVerifierResponse {
    ScramVerifierResponse {
        request_id: request.request_id,
        result: accounts.scram_verifier(&request.account),
    }
}

/// Загружает SHA-256 verifier только через единственного Accounts owner.
pub(super) fn load_sha256(
    accounts: &crate::accounts::Accounts,
    request: ScramSha256VerifierRequest,
) -> ScramSha256VerifierResponse {
    ScramSha256VerifierResponse {
        request_id: request.request_id,
        result: accounts.scram_sha256_verifier(&request.account),
    }
}
