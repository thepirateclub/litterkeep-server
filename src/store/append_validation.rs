//! Проверка mailbox-набора перед публикацией общего Store transaction.

use super::super::validated_unique_mailboxes;
use std::io;

/// Разрешает redirect-only transaction, но запрещает пустой обычный APPEND.
pub(super) fn validated_delivery_mailboxes(
    mailboxes: &[String],
    outbound: &[(String, String)],
) -> io::Result<Vec<String>> {
    match (mailboxes.is_empty(), outbound.is_empty()) {
        (false, _) => validated_unique_mailboxes(mailboxes),
        (true, false) => Ok(Vec::new()),
        (true, true) => Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients")),
    }
}
