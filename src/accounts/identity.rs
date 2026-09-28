//! Канонизация имён аккаунтов по SASLprep и проверка ограничения их размера.

use crate::sasl::saslprep::{self, SaslPrepContext};
use std::io;

/// Подготавливает новое имя аккаунта по stored-профилю RFC 4013.
pub(super) fn stored_name(name: &str) -> io::Result<String> {
    prepare_name(name, SaslPrepContext::StoredValue)
}

/// Подготавливает протокольный логин по query-профилю RFC 4013.
pub(super) fn query_name(name: &str) -> io::Result<String> {
    prepare_name(name, SaslPrepContext::Query)
}

/// Нормализует имя до проверки размера и запрещённых символов.
fn prepare_name(name: &str, context: SaslPrepContext) -> io::Result<String> {
    let prepared = saslprep::prepare(name, context).map_err(invalid_name)?;
    let invalid = prepared.is_empty()
        || prepared.len() > crate::smtp::MAX_LOCAL_PART_SIZE
        || prepared.chars().any(char::is_control);
    match invalid {
        true => Err(invalid_name("invalid account name")),
        false => Ok(prepared),
    }
}

/// Возвращает безопасную ошибку без включения исходного логина.
fn invalid_name(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}
