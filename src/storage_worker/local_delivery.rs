//! Атомарная локальная публикация SMTP recipients владельцем Store.

use crate::{accounts::Accounts, store::Store};
use std::io;

/// Сначала проверяет весь список получателей, затем публикует сообщение.
///
/// Предварительная проверка не допускает частичной доставки из-за опечатки в
/// последнем получателе. Ошибка ввода возвращается reactor-у без SMTP success.
pub(super) fn append_many(
    store: &mut Store,
    accounts: &Accounts,
    mailboxes: Vec<String>,
    envelope_sender: &str,
    bytes: &[u8],
) -> io::Result<u64> {
    if mailboxes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients"));
    }
    if !mailboxes.iter().all(|mailbox| accounts.exists(mailbox)) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "unknown local recipient",
        ));
    }
    store.deliver_sieve_many(&mailboxes, envelope_sender, bytes)
}
