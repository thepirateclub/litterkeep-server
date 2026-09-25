//! Атомарный append-only журнал изменяемых флагов сообщений.

use super::model::Record;
use super::{Store, durability};
use crate::mail_flags::{FlagMutation, KeywordFlag, MessageFlags, StoredFlags};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};

pub(super) fn load(
    root: &std::path::Path,
    entries: &HashMap<String, Vec<Record>>,
) -> io::Result<HashMap<String, HashMap<u64, StoredFlags>>> {
    let mut result = HashMap::new();
    for (mailbox, path) in
        super::file_key::entries(&root.join("mailboxes"), super::file_key::FileKind::Flags)?
    {
        let Some(records) = entries.get(&mailbox) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "flags without mailbox",
            ));
        };
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (flags, valid_length) = super::flag_journal::scan(&bytes, records)?;
        if valid_length < bytes.len() {
            let file = OpenOptions::new().write(true).open(path)?;
            file.set_len(valid_length as u64)?;
            file.sync_all()?;
        }
        result.insert(mailbox, flags);
    }
    Ok(result)
}

impl Store {
    /// Фиксирует изменение всех UID одним checksum-защищённым frame.
    pub fn update_flags(
        &mut self,
        mailbox: &str,
        uids: &[u64],
        operation: FlagMutation,
        operand: MessageFlags,
    ) -> io::Result<()> {
        self.update_stored_flags_system(mailbox, uids, operation, operand)
    }

    /// Меняет только системные bits, сохраняя keywords каждой записи.
    pub fn update_stored_flags_system(
        &mut self,
        mailbox: &str,
        uids: &[u64],
        operation: FlagMutation,
        operand: MessageFlags,
    ) -> io::Result<()> {
        self.update_flags_inner(mailbox, uids, operation, operand, None)
    }

    /// Атомарно заменяет системные флаги и пользовательские keywords.
    pub fn update_stored_flags(
        &mut self,
        mailbox: &str,
        uids: &[u64],
        operation: FlagMutation,
        system: MessageFlags,
        keywords: &BTreeSet<KeywordFlag>,
    ) -> io::Result<()> {
        self.update_flags_inner(mailbox, uids, operation, system, Some(keywords))
    }

    fn update_flags_inner(
        &mut self,
        mailbox: &str,
        uids: &[u64],
        operation: FlagMutation,
        system: MessageFlags,
        keywords: Option<&BTreeSet<KeywordFlag>>,
    ) -> io::Result<()> {
        self.ensure_writable()?;
        super::model::validate_mailbox(mailbox)?;
        let records = self.entries.get(mailbox).map(Vec::as_slice).unwrap_or(&[]);
        let mut unique_uids = uids.to_vec();
        unique_uids.sort_unstable();
        unique_uids.dedup();
        if unique_uids.iter().any(|uid| {
            records
                .binary_search_by_key(uid, |record| record.uid)
                .is_err()
        }) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "unknown message UID",
            ));
        }
        let existing = self.flags.get(mailbox);
        let updates = unique_uids
            .iter()
            .map(|uid| {
                let initial = records
                    .binary_search_by_key(uid, |record| record.uid)
                    .ok()
                    .and_then(|position| records.get(position))
                    .map(|record| StoredFlags {
                        system: record.flags,
                        keywords: record.keywords.clone(),
                    })
                    .unwrap_or_default();
                (
                    *uid,
                    existing
                        .and_then(|current| current.get(uid))
                        .cloned()
                        .unwrap_or(initial)
                        .apply(operation, system, keywords),
                )
            })
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return Ok(());
        }
        let frame = super::flag_journal::encode(&updates)?;
        let mailbox_root = self.root.join("mailboxes");
        let path =
            super::file_key::prepare(&mailbox_root, mailbox, super::file_key::FileKind::Flags)?;
        let durable = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&frame).and_then(|()| file.sync_all()))
            .and_then(|()| {
                let directory = path.parent().unwrap_or(&mailbox_root);
                durability::sync_directory(directory)?;
                if directory != mailbox_root {
                    durability::sync_directory(&mailbox_root)?;
                }
                Ok(())
            });
        if let Err(error) = durable {
            self.write_state = super::WriteState::Poisoned;
            return Err(error);
        }
        self.flags
            .entry(mailbox.to_owned())
            .or_default()
            .extend(updates);
        Ok(())
    }
}
