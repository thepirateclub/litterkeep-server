#[path = "store/append.rs"]
mod append;
#[path = "store/blobs.rs"]
mod blobs;
#[path = "store/commit_log.rs"]
mod commit_log;
#[path = "store/copy.rs"]
mod copy;
#[path = "store/durability.rs"]
mod durability;
#[cfg(test)]
#[path = "store/expunge_tests.rs"]
mod expunge_tests;
#[path = "store/expunges.rs"]
mod expunges;
#[cfg(test)]
#[path = "store/failure_matrix_tests.rs"]
mod failure_matrix_tests;
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
#[path = "store/move.rs"]
mod move_messages;
#[path = "store/open.rs"]
mod open;
#[cfg(all(test, target_os = "linux"))]
#[path = "store/outbound_service_tests.rs"]
mod outbound_service_tests;
#[cfg(test)]
#[path = "store/outbound_tests.rs"]
mod outbound_tests;
#[path = "store/quota.rs"]
mod quota;
#[path = "store/read.rs"]
mod read;
#[path = "store/recent.rs"]
mod recent;
#[path = "store/recent_access.rs"]
mod recent_access;
#[cfg(test)]
#[path = "store/recent_tests.rs"]
mod recent_tests;
#[path = "store/recovery.rs"]
mod recovery;
#[path = "store/sieve_access.rs"]
mod sieve_access;
#[path = "store/sieve_diagnostics.rs"]
mod sieve_diagnostics;
#[cfg(test)]
#[path = "store/sieve_diagnostics_tests.rs"]
mod sieve_diagnostics_tests;
#[path = "store/sieve_scripts.rs"]
mod sieve_scripts;
#[cfg(test)]
#[path = "store/sieve_tests.rs"]
mod sieve_tests;
#[path = "store/subscription_access.rs"]
mod subscription_access;
#[path = "store/subscriptions.rs"]
mod subscriptions;
#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
pub use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};
use crate::queue::Queue;
pub use folder_access::ResolvedMailbox;
pub use folders::{FolderInfo, SpecialUse};
pub use model::{AppendMetadata, MailboxSnapshot, Message};
use quota::Quota;
pub use sieve_diagnostics::SieveDiagnostic;
pub use sieve_scripts::ScriptInfo;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
pub use subscriptions::SubscribedFolder;
pub(super) const DEFAULT_BLOB_QUOTA: u64 = 25 * 1024 * 1024;
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    _writer_lock: File,
    next_uids: HashMap<String, u64>,
    entries: HashMap<String, Vec<model::Record>>,
    flags: HashMap<String, HashMap<u64, MessageFlags>>,
    /// UID, ещё не назначенные одной selected IMAP-сессии как `\\Recent`.
    recent: HashMap<String, std::collections::BTreeSet<u64>>,
    folders: folders::FolderCatalog,
    subscriptions: subscriptions::SubscriptionCatalog,
    sieve_scripts: sieve_scripts::SieveCatalog,
    /// Queue принадлежит тому же единственному writer-у, что и mailbox journal.
    queue: Queue,
    /// Единственный источник видимости queue jobs после recovery или commit.
    committed_transactions: HashSet<u64>,
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
    Stage(AppendStage),
}
/// Долговечные границы одной append transaction в порядке выполнения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppendStage {
    BlobPublished,
    QueueWritten,
    DiagnosticsWritten,
    FirstMailboxFrame,
    MailboxDirectorySynced,
    RecentWritten,
    CommitFrameSynced,
    CommitDirectorySynced,
    BeforeMemoryPublish,
}
impl Store {
    #[cfg(test)]
    fn set_failpoint(&mut self, failpoint: StoreFailpoint) {
        self.failpoint = Some(failpoint);
    }
    #[cfg(test)]
    fn check_failpoint(&self, stage: AppendStage) -> io::Result<()> {
        match self.failpoint {
            Some(StoreFailpoint::Stage(expected)) if expected == stage => {
                Err(io::Error::other(format!("injected {stage:?} failure")))
            }
            Some(StoreFailpoint::Stage(_)) | None => Ok(()),
        }
    }
    #[cfg(not(test))]
    fn check_failpoint(&self, _stage: AppendStage) -> io::Result<()> {
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
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
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
