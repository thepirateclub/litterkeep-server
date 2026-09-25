//! Публичная операция создания Sieve vacation-ответа.

use super::{
    Store,
    vacation_suppression::{PendingClaim, VacationKey},
};
use std::io;

/// Результат проверки durable suppression history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VacationResponseResult {
    Suppressed,
    Queued { transaction_id: u64 },
}

/// Проверенные параметры generated ответа для одной Store транзакции.
pub struct VacationResponseRequest<'a> {
    pub account: &'a str,
    pub sender: &'a str,
    pub response_identity: &'a str,
    pub recipient: &'a str,
    pub raw: &'a [u8],
    pub now_epoch: u64,
    pub days: u64,
}

impl Store {
    /// Атомарно ставит RFC822-ответ с пустым reverse-path и claim подавления.
    ///
    /// Вызывающий слой обязан передать нормализованный Sieve response identity
    /// и уже ограниченный политикой `days`; RFC guards и MIME-сборка выполняются
    /// выше, прежде чем эта долговечная операция будет вызвана.
    pub fn enqueue_vacation_response(
        &mut self,
        request: VacationResponseRequest<'_>,
    ) -> io::Result<VacationResponseResult> {
        self.ensure_writable()?;
        if request.days == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vacation days must be positive",
            ));
        }
        let seconds = request.days.checked_mul(86_400).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "vacation interval overflow")
        })?;
        let expires_at = request.now_epoch.checked_add(seconds).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "vacation expiry overflow")
        })?;
        let key = VacationKey {
            account: request.account.to_owned(),
            sender: request.sender.to_owned(),
            identity: request.response_identity.to_owned(),
        };
        super::vacation_suppression::validate(&key)?;
        match self.vacation_history.get(&key) {
            Some(expiration) if *expiration > request.now_epoch => {
                return Ok(VacationResponseResult::Suppressed);
            }
            Some(_) | None => {}
        }
        let claim = PendingClaim {
            key,
            expires_at,
            created_at: request.now_epoch,
        };
        let outbound = [(String::new(), request.recipient.to_owned())];
        let transaction_id = self.append_vacation_payload(request.raw, &outbound, &claim)?;
        Ok(VacationResponseResult::Queued { transaction_id })
    }

    fn append_vacation_payload(
        &mut self,
        raw: &[u8],
        outbound: &[(String, String)],
        claim: &PendingClaim,
    ) -> io::Result<u64> {
        self.append_vacation_transaction(raw, outbound, claim)
    }
}
