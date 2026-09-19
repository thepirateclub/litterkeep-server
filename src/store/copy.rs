//! Долговечная COPY-ссылка на уже immutable message blob.
//!
//! Один COPY фиксируется одной глобальной transaction. Destination frames
//! ссылаются на исходные blobs через отдельный blob transaction ID, поэтому
//! restart не может увидеть половину выбранного sequence-set.

use super::{Store, WriteState, model};
use std::io;

impl Store {
    /// Копирует известные UID в destination mailbox и возвращает новые UID.
    pub fn copy(&mut self, source: &str, destination: &str, uids: &[u64]) -> io::Result<Vec<u64>> {
        self.ensure_writable()?;
        model::validate_mailbox(source)?;
        model::validate_mailbox(destination)?;
        let source_records = self.copy_sources(source, uids)?;
        if source_records.is_empty() {
            return Ok(Vec::new());
        }
        let transaction_id = self.next_transaction_id;
        let links = self.prepare_copy_links(destination, transaction_id, source_records)?;
        let destination_uids = links.iter().map(|(_, record)| record.uid).collect();
        match self.commit_links(transaction_id, &links) {
            Ok(()) => self.publish_copy_links(transaction_id, destination, links, destination_uids),
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
    }

    /// Читает исходные записи до записи journal, чтобы неизвестный UID не
    /// создал ни одного destination frame.
    pub(super) fn copy_sources(
        &self,
        source: &str,
        uids: &[u64],
    ) -> io::Result<Vec<model::Record>> {
        let records = self.entries.get(source).map(Vec::as_slice).unwrap_or(&[]);
        let mut selected = Vec::with_capacity(uids.len());
        for uid in uids {
            let Some(record) = records.iter().find(|record| record.uid == *uid) else {
                return Err(io::Error::new(io::ErrorKind::NotFound, "unknown copy UID"));
            };
            let flags = self
                .flags
                .get(source)
                .and_then(|values| values.get(uid))
                .copied()
                .unwrap_or(record.flags);
            let mut copy = record.clone();
            copy.flags = flags;
            selected.push(copy);
        }
        Ok(selected)
    }

    /// Резервирует destination UID локально, не публикуя ссылку до commit.
    pub(super) fn prepare_copy_links(
        &self,
        destination: &str,
        transaction_id: u64,
        records: Vec<model::Record>,
    ) -> io::Result<Vec<(String, model::Record)>> {
        let mut next_uid = self.next_uids.get(destination).copied().unwrap_or(1);
        let mut links = Vec::with_capacity(records.len());
        for record in records {
            next_uid = next_uid
                .checked_add(1)
                .ok_or_else(|| io::Error::other("uid exhausted"))?;
            links.push((
                destination.to_owned(),
                model::Record {
                    uid: next_uid - 1,
                    transaction_id,
                    blob_transaction_id: record.blob_transaction_id,
                    name: record.name,
                    flags: record.flags,
                    internal_date: record.internal_date,
                },
            ));
        }
        Ok(links)
    }

    /// Публикует только уже durable links в materialized state writer-а.
    fn publish_copy_links(
        &mut self,
        transaction_id: u64,
        destination: &str,
        links: Vec<(String, model::Record)>,
        destination_uids: Vec<u64>,
    ) -> io::Result<Vec<u64>> {
        let next_uid = destination_uids
            .last()
            .copied()
            .and_then(|uid| uid.checked_add(1))
            .ok_or_else(|| io::Error::other("uid exhausted"))?;
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
    use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn copy_preserves_metadata_and_replays_shared_blob_links() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-copy-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        store.append("source", b"first").expect("first");
        store.append("source", b"second").expect("second");
        store
            .update_flags(
                "source",
                &[2],
                FlagMutation::Add,
                MessageFlags::from_flags([SystemFlag::Seen]),
            )
            .expect("flags");
        assert_eq!(
            store.copy("source", "destination", &[1, 2]).expect("copy"),
            [1, 2]
        );
        let source = store.snapshot("source").expect("source snapshot");
        let copied = store.snapshot("destination").expect("destination snapshot");
        assert_eq!(copied.messages.len(), 2);
        assert_eq!(
            copied.messages.first().map(|message| &message.bytes),
            Some(&b"first".to_vec())
        );
        assert!(
            copied
                .messages
                .last()
                .is_some_and(|message| message.flags.contains(SystemFlag::Seen))
        );
        assert_eq!(
            source.messages.first().map(|message| &message.path),
            copied.messages.first().map(|message| &message.path)
        );
        drop(store);

        let reopened = Store::open(&root).expect("reopen");
        let recovered = reopened.snapshot("destination").expect("recovered copy");
        assert_eq!(recovered.messages.len(), 2);
        assert_eq!(recovered.uid_next, 3);
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
