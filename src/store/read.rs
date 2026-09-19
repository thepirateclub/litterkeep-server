//! Построение согласованного read-only снимка mailbox.

use super::{MailboxSnapshot, Message, Store, model};
use std::fs;
use std::io;

impl Store {
    /// Читает метаданные и blobs из одного materialized view владельца store.
    pub fn snapshot(&self, mailbox: &str) -> io::Result<MailboxSnapshot> {
        model::validate_mailbox(mailbox)?;
        let messages = self
            .entries
            .get(mailbox)
            .into_iter()
            .flatten()
            .map(|record| {
                let path = self.root.join("blobs").join(&record.name);
                Ok(Message {
                    uid: record.uid,
                    bytes: fs::read(&path)?,
                    path,
                    flags: self
                        .flags
                        .get(mailbox)
                        .and_then(|flags| flags.get(&record.uid))
                        .copied()
                        .unwrap_or(record.flags),
                    internal_date: record.internal_date,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(MailboxSnapshot {
            // Значение остаётся постоянным, потому что mailbox journals пока не
            // удаляются и UID никогда не перераспределяются. Реализация DELETE
            // обязана сохранить allocator либо назначить новое значение.
            uid_validity: 1,
            uid_next: self.next_uids.get(mailbox).copied().unwrap_or(1),
            messages,
        })
    }

    /// Совместимый узкий API для POP3 и существующих store-тестов.
    pub fn messages(&self, mailbox: &str) -> io::Result<Vec<Message>> {
        self.snapshot(mailbox).map(|snapshot| snapshot.messages)
    }
}
