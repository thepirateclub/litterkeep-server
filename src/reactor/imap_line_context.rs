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

/// Представляет разобранные ссылки reactor с предметными именами.
pub(super) struct LineContextParts<'a> {
    pub(super) slot: usize,
    pub(super) generation: u64,
    pub(super) worker: &'a mut WakingStorageWorker,
    pub(super) owners: &'a mut HashMap<u64, (usize, u64, Pending)>,
    pub(super) request_id: &'a mut u64,
}

impl<'a> LineContext<'a> {
    /// Передаёт обработчику поля отдельно без копирования ссылок-владельцев.
    pub(super) fn into_parts(self) -> LineContextParts<'a> {
        LineContextParts {
            slot: self.slot,
            generation: self.generation,
            worker: self.worker,
            owners: self.owners,
            request_id: self.request_id,
        }
    }
}
