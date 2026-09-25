//! Атомарное IMAP MOVE поверх общего transaction-журнала хранилища.
//!
//! Сначала durable записываются destination links и tombstone источника. Обе
//! части становятся видимыми только после общей commit-записи, поэтому сбой
//! сохраняет старое состояние, а не полу-выполненное перемещение.

use super::{Store, WriteState, durability, expunges, model, recent};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{self, Write};

impl Store {
    /// Перемещает известные UID без промежуточного состояния copy-only.
    pub fn move_messages(
        &mut self,
        source: &str,
        destination: &str,
        uids: &[u64],
    ) -> io::Result<Vec<u64>> {
        self.ensure_writable()?;
        model::validate_mailbox(source)?;
        model::validate_mailbox(destination)?;
        if source == destination {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "same mailbox MOVE",
            ));
        }
        let source_records = self.copy_sources(source, uids)?;
        if source_records.is_empty() {
            return Ok(Vec::new());
        }
        let transaction_id = self.next_transaction_id;
        let links = self.prepare_copy_links(destination, transaction_id, source_records)?;
        let destination_uids = links.iter().map(|(_, record)| record.uid).collect();
        match self.commit_move(transaction_id, source, uids, &links) {
            Ok(()) => self.publish_move(
                transaction_id,
                source,
                destination,
                uids,
                links,
                destination_uids,
            ),
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
    }

    fn commit_move(
        &self,
        transaction_id: u64,
        source: &str,
        uids: &[u64],
        links: &[(String, model::Record)],
    ) -> io::Result<()> {
        let mailbox_root = self.root.join("mailboxes");
        for (mailbox, record) in links {
            let index = super::file_key::prepare(
                &mailbox_root,
                mailbox,
                super::file_key::FileKind::Journal,
            )?;
            let mut file = OpenOptions::new().create(true).append(true).open(&index)?;
            file.write_all(&super::journal::frame(record))?;
            file.sync_all()?;
            if let Some(parent) = index.parent()
                && parent != mailbox_root
            {
                durability::sync_directory(parent)?;
            }
        }
        expunges::write_transaction(&self.root, source, transaction_id, uids)?;
        durability::sync_directory(&self.root.join("mailboxes"))?;
        recent::add(&self.root, transaction_id, links)?;
        recent::drop_transaction(&self.root, transaction_id, source, uids)?;
        let mut commits = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("journal/transactions.log"))?;
        commits.write_all(&super::commit_log::frame(transaction_id))?;
        commits.sync_all()?;
        durability::sync_directory(&self.root.join("journal"))
    }

    fn publish_move(
        &mut self,
        transaction_id: u64,
        source: &str,
        destination: &str,
        source_uids: &[u64],
        links: Vec<(String, model::Record)>,
        destination_uids: Vec<u64>,
    ) -> io::Result<Vec<u64>> {
        let next_uid = destination_uids
            .last()
            .copied()
            .and_then(|uid| uid.checked_add(1))
            .ok_or_else(|| io::Error::other("uid exhausted"))?;
        let removed: HashSet<u64> = source_uids.iter().copied().collect();
        if let Some(records) = self.entries.get_mut(source) {
            records.retain(|record| !removed.contains(&record.uid));
        }
        if let Some(recent) = self.recent.get_mut(source) {
            recent.retain(|uid| !removed.contains(uid));
        }
        let records = self.entries.entry(destination.to_owned()).or_default();
        for (_, record) in links {
            self.recent
                .entry(destination.to_owned())
                .or_default()
                .insert(record.uid);
            records.push(record);
        }
        self.next_uids.insert(destination.to_owned(), next_uid);
        self.next_transaction_id = transaction_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?;
        Ok(destination_uids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn move_is_atomic_and_survives_restart() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-move-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        store.append("source", b"first").expect("first");
        store.append("source", b"second").expect("second");
        assert_eq!(
            store
                .move_messages("source", "destination", &[1])
                .expect("move"),
            [1]
        );
        assert_eq!(store.snapshot("source").expect("source").messages.len(), 1);
        assert_eq!(
            store
                .snapshot("destination")
                .expect("destination")
                .messages
                .len(),
            1
        );
        drop(store);

        let reopened = Store::open(&root).expect("reopen");
        assert_eq!(
            reopened.snapshot("source").expect("source").messages.len(),
            1
        );
        let destination = reopened.snapshot("destination").expect("destination");
        assert_eq!(
            destination.messages.first().map(|message| &message.bytes),
            Some(&b"first".to_vec())
        );
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
