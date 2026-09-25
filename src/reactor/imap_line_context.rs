//! Контекст владельца reactor для одной IMAP-команды.

use super::{Pending, WakingStorageWorker};
use std::collections::HashMap;

/// Передаёт обработчику команды только ссылки на уже принадлежащие reactor ресурсы.
pub(super) struct LineContext<'a> {
    pub(super) slot: usize,
    pub(super) generation: u64,
    pub(super) worker: &'a mut WakingStorageWorker,
    pub(super) owners: &'a mut HashMap<u64, (usize, u64, Pending)>,
    pub(super) request_id: &'a mut u64,
}
