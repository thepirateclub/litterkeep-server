//! Фиксация outbound recipients и исходной SMTP RCPT связи в durable queue.

use super::super::Store;
use std::io;

/// Проверяет параллельных outbound маршрутных и исходных RCPT списков.
pub(super) fn validate_count(
    outbound: &[(String, String)],
    original_indices: &[Vec<usize>],
) -> io::Result<()> {
    match original_indices.is_empty() || original_indices.len() == outbound.len() {
        true => Ok(()),
        false => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "outbound origin count mismatch",
        )),
    }
}

/// Записывает каждый redirect route до общей commit-точки Store transaction.
pub(super) fn enqueue(
    store: &Store,
    transaction_id: u64,
    outbound: &[(String, String)],
    original_indices: &[Vec<usize>],
) -> io::Result<()> {
    for (position, (sender, recipient)) in outbound.iter().enumerate() {
        store.queue.enqueue_owned_many(
            transaction_id,
            sender,
            recipient,
            original_indices.get(position).map_or(&[], Vec::as_slice),
        )?;
    }
    Ok(())
}
