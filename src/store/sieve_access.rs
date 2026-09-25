//! Публичные Store-операции Sieve без раскрытия внутреннего каталога.

use super::{ScriptInfo, SieveDiagnostic, Store, sieve_diagnostics};
use crate::sieve;
use std::collections::HashSet;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

struct GeneratedVacation {
    account: String,
    sender: String,
    identity: String,
    recipient: String,
    payload: Vec<u8>,
    now_epoch: u64,
    days: u64,
}

impl Store {
    /// Валидирует и атомарно публикует новую версию named script.
    pub fn put_sieve_script(
        &mut self,
        account: &str,
        name: &str,
        source: String,
    ) -> io::Result<ScriptInfo> {
        self.sieve_scripts.put(account, name, source)
    }

    /// Меняет active script только после durable snapshot replacement.
    pub fn activate_sieve_script(&mut self, account: &str, name: Option<&str>) -> io::Result<()> {
        self.sieve_scripts.activate(account, name)
    }

    /// Возвращает неизменяемую версию script для MANAGESIEVE GETSCRIPT.
    pub fn sieve_script(&self, account: &str, name: &str) -> io::Result<(u64, String)> {
        self.sieve_scripts.source(account, name)
    }

    /// Фиксирует active source и его version перед delivery transaction.
    pub fn active_sieve_script(&self, account: &str) -> io::Result<Option<(u64, String)>> {
        self.sieve_scripts.active(account)
    }

    /// Отдаёт только метаданные scripts текущего владельца account.
    pub fn sieve_scripts(&self, account: &str) -> Vec<ScriptInfo> {
        self.sieve_scripts.list(account)
    }

    /// Удаляет только inactive script, сохраняя явно выбранную active версию.
    pub fn delete_sieve_script(&mut self, account: &str, name: &str) -> io::Result<()> {
        self.sieve_scripts.delete(account, name)
    }

    /// Переименовывает script atomically и переносит active reference при нужде.
    pub fn rename_sieve_script(&mut self, account: &str, old: &str, new: &str) -> io::Result<()> {
        self.sieve_scripts.rename(account, old, new)
    }

    /// Проверяет доступное место без изменения versioned state.
    pub fn sieve_have_space(&self, account: &str, name: &str, size: u64) -> io::Result<bool> {
        self.sieve_scripts.have_space(account, name, size)
    }

    /// Проверяет source тем же parser/validator, что и PUTSCRIPT.
    pub fn check_sieve_script(&self, source: &str) -> io::Result<()> {
        sieve::parse(source).map(|_| ()).map_err(io::Error::other)
    }

    /// Возвращает долговечные runtime-ошибки только committed доставок account.
    pub fn sieve_diagnostics(&self, account: &str) -> io::Result<Vec<SieveDiagnostic>> {
        sieve_diagnostics::load(&self.root, &self.committed_transactions, account)
    }

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
            _ => Some(self.append_many_with_sieve(&mailboxes, raw, &outbound, &diagnostics)?),
        };
        for response in vacation_responses {
            let result = self.enqueue_vacation_response(super::VacationResponseRequest {
                account: &response.account,
                sender: &response.sender,
                response_identity: &response.identity,
                recipient: &response.recipient,
                raw: &response.payload,
                now_epoch: response.now_epoch,
                days: response.days,
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
        days: sieve::effective_days(action.days),
    }))
}

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

/// Одинаковая allowlist проверяется до queue и повторно перед transport socket.
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

/// RFC-safe fallback не исполняет частично построенный план после runtime error.
fn implicit_keep() -> sieve::DeliveryPlan {
    sieve::DeliveryPlan {
        keep: true,
        discard: false,
        folders: Vec::new(),
        redirects: Vec::new(),
        vacations: Vec::new(),
    }
}

/// Malformed MIME не отменяет уже принятую SMTP-доставку: правило не совпадает.
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

#[cfg(test)]
#[path = "sieve_access_tests.rs"]
mod tests;
