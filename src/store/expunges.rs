//! Долговечные tombstone-события окончательного удаления сообщений IMAP.

use super::durability;
use super::expunge_codec::{self, encode_legacy, encode_transaction};
use super::model::Record;
use super::{Store, WriteState, commit_log};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Загружает tombstones после mailbox journals и скрывает удалённые UID.
pub(super) fn load(
    root: &Path,
    entries: &mut HashMap<String, Vec<Record>>,
    committed: &commit_log::Committed,
) -> io::Result<()> {
    for (mailbox, records) in entries.iter_mut() {
        let path = super::file_key::path(
            &root.join("mailboxes"),
            mailbox,
            super::file_key::FileKind::Expunges,
        );
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (uids, valid_length) = expunge_codec::decode_all(&bytes, records, committed)?;
        if valid_length < bytes.len() {
            let file = OpenOptions::new().write(true).open(&path)?;
            file.set_len(valid_length as u64)?;
            file.sync_all()?;
        }
        let removed: HashSet<u64> = uids.into_iter().collect();
        records.retain(|record| !removed.contains(&record.uid));
    }
    Ok(())
}

impl Store {
    /// Фиксирует окончательное удаление известных UID одним fsync frame.
    pub fn expunge(&mut self, mailbox: &str, uids: &[u64]) -> io::Result<Vec<u64>> {
        self.ensure_writable()?;
        super::model::validate_mailbox(mailbox)?;
        let records = self.entries.get(mailbox).map(Vec::as_slice).unwrap_or(&[]);
        let mut removed = uids.to_vec();
        removed.sort_unstable();
        removed.dedup();
        if removed
            .iter()
            .any(|uid| !records.iter().any(|record| record.uid == *uid))
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "unknown expunge UID",
            ));
        }
        if removed.is_empty() {
            return Ok(removed);
        }
        let removed_set: HashSet<u64> = removed.iter().copied().collect();
        let frame = encode_legacy(&removed)?;
        let mailbox_root = self.root.join("mailboxes");
        let path =
            super::file_key::prepare(&mailbox_root, mailbox, super::file_key::FileKind::Expunges)?;
        let durable = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&frame).and_then(|()| file.sync_all()))
            .and_then(|()| sync_key_directory(&mailbox_root, &path));
        if let Err(error) = durable {
            self.write_state = WriteState::Poisoned;
            return Err(error);
        }
        self.discard_recent(mailbox, &removed)
            .map_err(|error| self.poison_after_recent_error(error))?;
        if let Some(records) = self.entries.get_mut(mailbox) {
            records.retain(|record| !removed_set.contains(&record.uid));
        }
        Ok(removed)
    }
}

/// Добавляет durable MOVE tombstone в журнал нужного mailbox-ключа.
pub(super) fn write_transaction(
    root: &Path,
    mailbox: &str,
    transaction_id: u64,
    uids: &[u64],
) -> io::Result<()> {
    let frame = encode_transaction(transaction_id, uids)?;
    let mailbox_root = root.join("mailboxes");
    let path =
        super::file_key::prepare(&mailbox_root, mailbox, super::file_key::FileKind::Expunges)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(&frame)?;
    file.sync_all()?;
    sync_key_directory(&mailbox_root, &path)
}

fn sync_key_directory(root: &Path, path: &Path) -> io::Result<()> {
    let directory = path.parent().unwrap_or(root);
    durability::sync_directory(directory)?;
    if directory == root {
        Ok(())
    } else {
        durability::sync_directory(root)
    }
}
