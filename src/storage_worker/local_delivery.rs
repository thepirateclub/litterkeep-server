//! Атомарная локальная публикация SMTP recipients владельцем Store.

use super::Response;
use crate::{accounts::Accounts, store::Store};
use std::io;

/// Формирует completion для единичного атомарного SMTP local-delivery запроса.
pub(super) fn append_many_response(
    store: &mut Store,
    accounts: &Accounts,
    request_id: u64,
    mailboxes: Vec<String>,
    envelope_recipients: Vec<String>,
    envelope_sender: String,
    bytes: Vec<u8>,
) -> Response {
    Response::Appended {
        request_id,
        result: append_many(
            store,
            accounts,
            mailboxes,
            envelope_recipients,
            &envelope_sender,
            &bytes,
        ),
    }
}

/// Сначала проверяет весь список получателей, затем публикует сообщение.
///
/// Предварительная проверка не допускает частичной доставки из-за опечатки в
/// последнем получателе. Ошибка ввода возвращается reactor-у без SMTP success.
pub(super) fn append_many(
    store: &mut Store,
    accounts: &Accounts,
    mailboxes: Vec<String>,
    envelope_recipients: Vec<String>,
    envelope_sender: &str,
    bytes: &[u8],
) -> io::Result<u64> {
    if mailboxes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients"));
    }
    if mailboxes.len() != envelope_recipients.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "recipient envelope count mismatch",
        ));
    }
    if !mailboxes
        .iter()
        .all(|mailbox| crate::local_route::known_recipient(mailbox, |name| accounts.exists(name)))
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "unknown local recipient",
        ));
    }
    store.deliver_sieve_many(&mailboxes, envelope_sender, &envelope_recipients, bytes)
}
