//! Атомарная локальная публикация SMTP recipients владельцем Store.

use super::{Response, SmtpDeliveryRequest};
use crate::{accounts::Accounts, store::Store};
use std::io;

/// Формирует completion для единичного атомарного SMTP local-delivery запроса.
pub(super) fn append_many_response(
    store: &mut Store,
    accounts: &Accounts,
    request: SmtpDeliveryRequest,
) -> Response {
    Response::Appended {
        request_id: request.request_id,
        result: append_many(store, accounts, &request),
    }
}

/// Сначала проверяет весь список получателей, затем публикует сообщение.
///
/// Предварительная проверка не допускает частичной доставки из-за опечатки в
/// последнем получателе. Ошибка ввода возвращается reactor-у без SMTP success.
pub(super) fn append_many(
    store: &mut Store,
    accounts: &Accounts,
    request: &SmtpDeliveryRequest,
) -> io::Result<u64> {
    if request.mailboxes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients"));
    }
    if request.mailboxes.len() != request.envelope_recipients.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "recipient envelope count mismatch",
        ));
    }
    if !request
        .mailboxes
        .iter()
        .all(|mailbox| crate::local_route::known_recipient(mailbox, |name| accounts.exists(name)))
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "unknown local recipient",
        ));
    }
    let dsn = match request.dsn.has_dsn_parameters() {
        true => Some(&request.dsn),
        false => None,
    };
    store.deliver_sieve_many_with_dsn(
        &request.mailboxes,
        &request.envelope_sender,
        &request.envelope_recipients,
        &request.bytes,
        dsn,
    )
}
