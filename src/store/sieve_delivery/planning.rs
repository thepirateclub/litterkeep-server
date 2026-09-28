//! Подготовка безопасного Sieve delivery plan и его fallback диагностики.

use super::super::{Store, sieve_diagnostics};
use crate::sieve;
use std::io;

/// Читает active revision и исполняет скрипт без частичной публикации.
pub(super) fn delivery_plan(
    store: &Store,
    account: &str,
    headers: &[(String, String)],
    message_size: usize,
    envelope_from: &str,
    envelope_to: &str,
) -> io::Result<(
    sieve::DeliveryPlan,
    Option<sieve_diagnostics::PendingDiagnostic>,
)> {
    let Some((version, source)) = store.active_sieve_script(account)? else {
        return Ok((implicit_keep(), None));
    };
    let script = sieve::parse(&source).map_err(io::Error::other)?;
    match sieve::execute_with_envelope(&script, headers, message_size, envelope_from, envelope_to) {
        Ok(mut plan) => match apply_redirect_policy(&mut plan, headers) {
            Some(error) => Ok(failed_plan(account, version, error)),
            None => Ok((plan, None)),
        },
        Err(error) => Ok(failed_plan(account, version, error)),
    }
}

/// Превращает runtime error в implicit keep и durable typed diagnostic.
fn failed_plan(
    account: &str,
    script_version: u64,
    error: sieve::ExecutionError,
) -> (
    sieve::DeliveryPlan,
    Option<sieve_diagnostics::PendingDiagnostic>,
) {
    (
        implicit_keep(),
        Some(sieve_diagnostics::PendingDiagnostic {
            account: account.to_owned(),
            script_version,
            error,
        }),
    )
}

/// Проверяет redirect allowlist и ограничивает число последовательных hops.
fn apply_redirect_policy(
    plan: &mut sieve::DeliveryPlan,
    headers: &[(String, String)],
) -> Option<sieve::ExecutionError> {
    let mut canonical = Vec::with_capacity(plan.redirects.len());
    for address in &plan.redirects {
        match crate::redirect_policy::Recipient::parse(address) {
            Ok(recipient) => canonical.push(recipient.canonical()),
            Err(_) => return Some(sieve::ExecutionError::RedirectRouteDenied),
        }
    }
    plan.redirects = canonical;
    match (
        plan.redirects.is_empty(),
        crate::redirect_policy::hop_limit_reached(headers),
    ) {
        (false, true) => Some(sieve::ExecutionError::RedirectLoop),
        _ => None,
    }
}

/// RFC-safe fallback не исполняет частично построенный план после ошибки.
fn implicit_keep() -> sieve::DeliveryPlan {
    sieve::DeliveryPlan {
        keep: true,
        discard: false,
        folders: Vec::new(),
        redirects: Vec::new(),
        vacations: Vec::new(),
    }
}

/// Извлекает UTF-8 заголовки для Sieve без срыва SMTP при malformed MIME.
pub(super) fn delivery_headers(raw: &[u8]) -> Vec<(String, String)> {
    match crate::mime::header_block::parse(raw) {
        Ok(block) => block
            .fields
            .into_iter()
            .filter_map(|field| {
                String::from_utf8(field.value)
                    .ok()
                    .map(|value| (field.name, value))
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
