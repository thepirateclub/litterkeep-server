//! Построение атомарного delivery plan поверх Sieve и durable Store.

use super::{Store, sieve_diagnostics};
use crate::sieve;
use std::collections::HashSet;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// Подготовленный vacation response до durable queue publication.
struct GeneratedVacation {
    account: String,
    sender: String,
    identity: String,
    recipient: String,
    payload: Vec<u8>,
    now_epoch: u64,
    interval_seconds: u64,
}

impl Store {
    /// Строит планы всех recipients до первой записи mailbox journal.
    ///
    /// Active source читается вместе с его версией у единственного Store owner,
    /// поэтому доставка не может увидеть частично заменённый script. Redirect
    /// записывается queue job до той же global commit точки, что и mailbox link.
    pub fn deliver_sieve_many(
        &mut self,
        accounts: &[String],
        envelope_sender: &str,
        envelope_recipients: &[String],
        raw: &[u8],
    ) -> io::Result<u64> {
        self.deliver_sieve_many_with_dsn(accounts, envelope_sender, envelope_recipients, raw, None)
    }

    /// Сохраняет DSN envelope вместе с общей transaction ID local delivery.
    pub(crate) fn deliver_sieve_many_with_dsn(
        &mut self,
        accounts: &[String],
        envelope_sender: &str,
        envelope_recipients: &[String],
        raw: &[u8],
        smtp_dsn: Option<&crate::smtp::DsnEnvelope>,
    ) -> io::Result<u64> {
        if accounts.len() != envelope_recipients.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "recipient envelope count mismatch",
            ));
        }
        if !accounts
            .iter()
            .zip(envelope_recipients)
            .all(|(account, address)| {
                crate::local_route::local_recipient(address).as_deref() == Some(account)
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "recipient envelope does not match local mailbox",
            ));
        }
        if let Some(envelope) = smtp_dsn
            && !smtp_dsn_validation::matches(envelope, envelope_sender, envelope_recipients)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "DSN envelope does not match SMTP transaction",
            ));
        }
        self.deliver_sieve_many_validated(
            accounts,
            envelope_sender,
            envelope_recipients,
            raw,
            smtp_dsn,
        )
    }

    /// Продолжает delivery после проверки границ typed SMTP envelope.
    fn deliver_sieve_many_validated(
        &mut self,
        accounts: &[String],
        envelope_sender: &str,
        envelope_recipients: &[String],
        raw: &[u8],
        smtp_dsn: Option<&crate::smtp::DsnEnvelope>,
    ) -> io::Result<u64> {
        let headers = delivery_headers(raw);
        let mut mailboxes = Vec::new();
        let mut outbound = Vec::new();
        let mut diagnostics = Vec::new();
        let mut vacation_responses = Vec::new();
        let mut evaluated = HashSet::with_capacity(accounts.len());
        for (account, envelope_to) in accounts.iter().zip(envelope_recipients) {
            if !evaluated.insert(account.as_str()) {
                continue;
            }
            let (plan, diagnostic) =
                self.delivery_plan(account, &headers, raw.len(), envelope_sender, envelope_to)?;
            if let Some(value) = diagnostic {
                diagnostics.push(value);
            }
            for recipient in plan.redirects {
                outbound.push((envelope_sender.to_owned(), recipient));
            }
            for action in plan.vacations {
                if let Some(response) =
                    prepare_vacation(account, envelope_sender, envelope_to, &headers, &action)?
                {
                    vacation_responses.push(response);
                }
            }
            if plan.keep {
                mailboxes.push(self.resolve_folder(account, "INBOX")?);
            }
            for folder in plan.folders {
                mailboxes.push(self.resolve_folder(account, &folder)?);
            }
        }
        let mut transaction_id = match (mailboxes.is_empty(), outbound.is_empty()) {
            (true, true) => None,
            _ => Some(self.append_many_with_sieve(
                &mailboxes,
                raw,
                &outbound,
                &diagnostics,
                smtp_dsn,
            )?),
        };
        for response in vacation_responses {
            let result = self.enqueue_vacation_response(super::VacationResponseRequest {
                account: &response.account,
                sender: &response.sender,
                response_identity: &response.identity,
                recipient: &response.recipient,
                raw: &response.payload,
                now_epoch: response.now_epoch,
                interval_seconds: response.interval_seconds,
            });
            let result = match result {
                Ok(result) => result,
                Err(error) => {
                    // Исходное письмо уже committed: ошибка автоответа не должна
                    // заставлять SMTP-клиента повторять доставку исходного письма.
                    eprintln!("Sieve vacation response was not queued: {error}");
                    continue;
                }
            };
            if let super::VacationResponseResult::Queued {
                transaction_id: queued,
            } = result
            {
                transaction_id.get_or_insert(queued);
            }
        }
        Ok(transaction_id.unwrap_or_default())
    }

    /// Читает и исполняет зафиксированную версию скрипта владельца mailbox.
    fn delivery_plan(
        &self,
        account: &str,
        headers: &[(String, String)],
        message_size: usize,
        envelope_from: &str,
        envelope_to: &str,
    ) -> io::Result<(
        sieve::DeliveryPlan,
        Option<sieve_diagnostics::PendingDiagnostic>,
    )> {
        let Some((version, source)) = self.active_sieve_script(account)? else {
            return Ok((implicit_keep(), None));
        };
        let script = sieve::parse(&source).map_err(io::Error::other)?;
        match sieve::execute_with_envelope(
            &script,
            headers,
            message_size,
            envelope_from,
            envelope_to,
        ) {
            Ok(mut plan) => match apply_redirect_policy(&mut plan, headers) {
                Some(error) => Ok(failed_plan(account, version, error)),
                None => Ok((plan, None)),
            },
            Err(error) => Ok(failed_plan(account, version, error)),
        }
    }
}

#[path = "sieve_delivery/smtp_dsn_validation.rs"]
mod smtp_dsn_validation;

/// Подготавливает только допустимый vacation response с обратным путём.
fn prepare_vacation(
    account: &str,
    envelope_sender: &str,
    envelope_recipient: &str,
    headers: &[(String, String)],
    action: &sieve::VacationAction,
) -> io::Result<Option<GeneratedVacation>> {
    if !sieve::may_respond(envelope_sender, headers) {
        return Ok(None);
    }
    if !sieve::personal_message(envelope_recipient, &action.addresses, headers) {
        return Ok(None);
    }
    let recipient = match crate::redirect_policy::Recipient::parse(envelope_sender) {
        Ok(address) => address.canonical(),
        Err(_) => return Ok(None),
    };
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    let identity = sieve::response_identity(action)?;
    let payload =
        sieve::build_vacation_response(action, envelope_recipient, &recipient, headers, now_epoch)?;
    Ok(Some(GeneratedVacation {
        account: account.to_owned(),
        sender: recipient,
        identity,
        recipient: envelope_sender.to_owned(),
        payload,
        now_epoch,
        interval_seconds: sieve::effective_seconds(action.interval),
    }))
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
fn delivery_headers(raw: &[u8]) -> Vec<(String, String)> {
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
