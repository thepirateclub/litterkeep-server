//! Проверка достижимости immutable blobs из согласованного Store snapshot.

use super::{Store, model};
use crate::maintenance::CheckReport;
use std::collections::HashSet;
use std::fs;
use std::io;

impl Store {
    /// Читает каждый referenced blob и отдельно учитывает безопасные orphan-ы.
    pub(crate) fn integrity_report(&self) -> io::Result<CheckReport> {
        let mut referenced = HashSet::new();
        let mut message_links = 0_usize;
        for records in self.entries.values() {
            for record in records {
                fs::read(self.root.join("blobs").join(&record.name))?;
                referenced.insert(record.name.clone());
                message_links = message_links
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("message link count overflow"))?;
            }
        }
        let pending = self.pending_outbound()?;
        for delivery in &pending {
            let name = model::blob_name(delivery.transaction_id);
            fs::read(self.root.join("blobs").join(&name))?;
            referenced.insert(name);
        }
        let mut orphan_blobs = 0_usize;
        for entry in fs::read_dir(self.root.join("blobs"))? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "non-file entry in blob directory",
                ));
            }
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "non-utf8 blob name"))?;
            if !referenced.contains(name) {
                orphan_blobs = orphan_blobs
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("orphan count overflow"))?;
            }
        }
        Ok(CheckReport {
            mailboxes: self.entries.len(),
            message_links,
            referenced_blobs: referenced.len(),
            orphan_blobs,
            pending_deliveries: pending.len(),
        })
    }
}
