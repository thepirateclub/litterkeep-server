//! Изменение materialized recent-состояния после долговечного удаления.
//!
//! Журнал и его codec остаются в `recent.rs`; этот файл отвечает только за
//! согласование уже опубликованного Store view с обычным IMAP EXPUNGE.

use super::{Store, WriteState, recent};
use std::collections::HashSet;
use std::io;

impl Store {
    /// Сначала фиксирует drop-event, затем меняет только memory projection.
    pub(super) fn discard_recent(&mut self, mailbox: &str, uids: &[u64]) -> io::Result<()> {
        recent::drop_immediate(&self.root, mailbox, uids)?;
        let removed = uids.iter().copied().collect::<HashSet<_>>();
        if let Some(values) = self.recent.get_mut(mailbox) {
            values.retain(|uid| !removed.contains(uid));
        }
        Ok(())
    }

    /// Не позволяет продолжить выдачу результатов, если durable drop не записан.
    pub(super) fn poison_after_recent_error(&mut self, error: io::Error) -> io::Error {
        self.write_state = WriteState::Poisoned;
        error
    }
}
