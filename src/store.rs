//! Filesystem store facade. The writer owns this value exclusively.
#[path = "store/blobs.rs"]
mod blobs;
#[path = "store/commit_log.rs"]
mod commit_log;
#[path = "store/copy.rs"]
mod copy;
#[path = "store/durability.rs"]
mod durability;
#[path = "store/expunges.rs"]
mod expunges;
#[path = "store/flags.rs"]
mod flags;
#[path = "store/folder_access.rs"]
mod folder_access;
#[path = "store/folder_cursor.rs"]
mod folder_cursor;
#[path = "store/folder_journal.rs"]
mod folder_journal;
#[cfg(test)]
#[path = "store/folder_journal_tests.rs"]
mod folder_journal_tests;
#[path = "store/folders.rs"]
mod folders;
#[path = "store/format.rs"]
mod format;
#[path = "store/journal.rs"]
mod journal;
#[path = "store/mailbox_state.rs"]
mod mailbox_state;
#[path = "store/model.rs"]
mod model;
#[path = "store/open.rs"]
mod open;
#[path = "store/quota.rs"]
mod quota;
#[path = "store/read.rs"]
mod read;
#[path = "store/recovery.rs"]
mod recovery;
#[path = "store/subscription_access.rs"]
mod subscription_access;
#[path = "store/subscriptions.rs"]
mod subscriptions;
#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
pub use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};
pub use folder_access::ResolvedMailbox;
pub use folders::{FolderInfo, SpecialUse};
pub use model::{AppendMetadata, MailboxSnapshot, Message};
use quota::Quota;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
pub use subscriptions::SubscribedFolder;
/// Начальный общий budget неизменяемых message blobs: 25 MiB.
pub(super) const DEFAULT_BLOB_QUOTA: u64 = 25 * 1024 * 1024;
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    _writer_lock: File,
    next_uids: HashMap<String, u64>,
    entries: HashMap<String, Vec<model::Record>>,
    flags: HashMap<String, HashMap<u64, MessageFlags>>,
    folders: folders::FolderCatalog,
    subscriptions: subscriptions::SubscriptionCatalog,
    next_transaction_id: u64,
    blob_quota: Quota,
    write_state: WriteState,
    #[cfg(test)]
    failpoint: Option<StoreFailpoint>,
}
#[derive(Debug, Eq, PartialEq)]
enum WriteState {
    Writable,
    Poisoned,
}
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
enum StoreFailpoint {
    AfterFirstMailboxFrame,
}
impl Store {
    pub fn append(&mut self, mailbox: &str, raw: &[u8]) -> io::Result<u64> {
        self.append_many(&[mailbox.to_owned()], raw)
    }
    /// APPEND фиксирует UID и начальные metadata одним mailbox frame.
    pub fn append_with_metadata(
        &mut self,
        mailbox: &str,
        raw: &[u8],
        metadata: AppendMetadata,
    ) -> io::Result<u64> {
        self.append_many_with_metadata(&[mailbox.to_owned()], raw, metadata)
    }
    /// Фиксирует одну delivery unit для всех уникальных mailbox одним commit.
    pub fn append_many(&mut self, mailboxes: &[String], raw: &[u8]) -> io::Result<u64> {
        self.append_many_with_metadata(mailboxes, raw, AppendMetadata::default())
    }

    fn append_many_with_metadata(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        metadata: AppendMetadata,
    ) -> io::Result<u64> {
        self.ensure_writable()?;
        let mailboxes = validated_unique_mailboxes(mailboxes)?;
        self.blob_quota.ensure_available(raw.len())?;
        let mut transaction_id = self.next_transaction_id;
        let (name, _) = blobs::next_available(&self.root, &mut transaction_id)?;
        let links = self.prepare_links(&mailboxes, transaction_id, &name, metadata)?;
        blobs::publish(&self.root, &name, raw)?;
        self.blob_quota.record(raw.len())?;
        match self.commit_links(transaction_id, &links) {
            Ok(()) => self.publish_links(transaction_id, links),
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
    }
    fn index(&self, mailbox: &str) -> PathBuf {
        self.root
            .join("mailboxes")
            .join(format!("{mailbox}.journal"))
    }

    fn ensure_writable(&self) -> io::Result<()> {
        match self.write_state {
            WriteState::Writable => Ok(()),
            WriteState::Poisoned => Err(io::Error::other("store writer is poisoned")),
        }
    }

    /// Резервирует UID только в памяти; видимость не меняется до commit frame.
    fn prepare_links(
        &self,
        mailboxes: &[String],
        transaction_id: u64,
        name: &str,
        metadata: AppendMetadata,
    ) -> io::Result<Vec<(String, model::Record)>> {
        mailboxes
            .iter()
            .map(|mailbox| {
                let uid = self.next_uids.get(mailbox).copied().unwrap_or(1);
                uid.checked_add(1)
                    .ok_or_else(|| io::Error::other("uid exhausted"))?;
                Ok((
                    mailbox.clone(),
                    model::Record {
                        uid,
                        transaction_id,
                        blob_transaction_id: transaction_id,
                        name: name.to_owned(),
                        flags: metadata.flags,
                        internal_date: metadata.internal_date,
                    },
                ))
            })
            .collect()
    }

    /// Пишет все mailbox links, затем единственную точку commit authority.
    fn commit_links(
        &self,
        transaction_id: u64,
        links: &[(String, model::Record)],
    ) -> io::Result<()> {
        let mut written = 0;
        for (mailbox, record) in links {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.index(mailbox))?;
            file.write_all(&journal::frame(record))?;
            file.sync_all()?;
            written += 1;
            self.check_mailbox_failpoint(written)?;
        }
        durability::sync_directory(&self.root.join("mailboxes"))?;
        let mut commits = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("journal/transactions.log"))?;
        commits.write_all(&commit_log::frame(transaction_id))?;
        commits.sync_all()?;
        durability::sync_directory(&self.root.join("journal"))
    }

    /// Обновляет materialized view только после durable global commit.
    fn publish_links(
        &mut self,
        transaction_id: u64,
        links: Vec<(String, model::Record)>,
    ) -> io::Result<u64> {
        let first_uid = links
            .first()
            .map(|(_, record)| record.uid)
            .ok_or_else(|| io::Error::other("delivery has no mailbox links"))?;
        for (mailbox, record) in links {
            let next_uid = record
                .uid
                .checked_add(1)
                .ok_or_else(|| io::Error::other("uid exhausted"))?;
            self.next_uids.insert(mailbox.clone(), next_uid);
            self.entries.entry(mailbox).or_default().push(record);
        }
        self.next_transaction_id = transaction_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?;
        Ok(first_uid)
    }
    #[cfg(test)]
    fn set_failpoint(&mut self, failpoint: StoreFailpoint) {
        self.failpoint = Some(failpoint);
    }
    #[cfg(test)]
    fn check_mailbox_failpoint(&self, written: usize) -> io::Result<()> {
        if let (Some(StoreFailpoint::AfterFirstMailboxFrame), 1) = (self.failpoint, written) {
            return Err(io::Error::other("injected mailbox frame failure"));
        }
        Ok(())
    }
    #[cfg(not(test))]
    fn check_mailbox_failpoint(&self, _written: usize) -> io::Result<()> {
        Ok(())
    }
}

/// Проверяет и дедуплицирует RCPT, сохраняя порядок первого появления.
fn validated_unique_mailboxes(mailboxes: &[String]) -> io::Result<Vec<String>> {
    if mailboxes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients"));
    }
    let mut seen = HashSet::new();
    let mut unique = Vec::with_capacity(mailboxes.len());
    for mailbox in mailboxes {
        model::validate_mailbox(mailbox)?;
        if seen.insert(mailbox.as_str()) {
            unique.push(mailbox.clone());
        }
    }
    Ok(unique)
}
#[cfg(target_os = "linux")]
fn open_writer_lock(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new().write(true).create(true).open(path)?;
    // Нормализуем платформенный EWOULDBLOCK к единому контракту хранилища:
    // занятый каталог уже принадлежит другому writer.
    if let Err(error) = crate::platform::linux_lock::acquire(&file) {
        return match error.kind() {
            io::ErrorKind::WouldBlock => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "store writer is already active",
            )),
            _ => Err(error),
        };
    }
    Ok(file)
}
#[cfg(not(target_os = "linux"))]
impl Drop for Store {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root.join("writer.lock"));
    }
}
#[cfg(not(target_os = "linux"))]
fn open_writer_lock(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}
