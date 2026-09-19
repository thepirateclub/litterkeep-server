//! Filesystem store facade. The writer owns this value exclusively.
#[path = "store/blobs.rs"]
mod blobs;
#[path = "store/durability.rs"]
mod durability;
#[path = "store/format.rs"]
mod format;
#[path = "store/journal.rs"]
mod journal;
#[path = "store/mailbox_state.rs"]
mod mailbox_state;
#[path = "store/model.rs"]
mod model;
#[path = "store/quota.rs"]
mod quota;
#[path = "store/recovery.rs"]
mod recovery;
#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
pub use model::Message;
use quota::Quota;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Начальный общий budget неизменяемых message blobs: 25 MiB.
const DEFAULT_BLOB_QUOTA: u64 = 25 * 1024 * 1024;
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    _writer_lock: File,
    next_uids: HashMap<String, u64>,
    blob_quota: Quota,
}
impl Store {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with_quota(root.as_ref(), DEFAULT_BLOB_QUOTA)
    }

    /// Открывает store с явным budget; отдельный параметр нужен для граничных тестов.
    fn open_with_quota(root: &Path, quota_limit: u64) -> io::Result<Self> {
        let root = root.to_path_buf();
        fs::create_dir_all(&root)?;
        format::ensure(&root)?;
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("mailboxes"))?;
        durability::sync_directory(&root)?;
        let lock = open_writer_lock(&root.join("writer.lock"))?;
        recovery::recover(&root)?;
        let next_uids = mailbox_state::load_next_uids(&root)?;
        let blob_quota = Quota::load(&root.join("blobs"), quota_limit)?;
        let store = Self {
            root,
            _writer_lock: lock,
            next_uids,
            blob_quota,
        };
        Ok(store)
    }
    pub fn append(&mut self, mailbox: &str, raw: &[u8]) -> io::Result<u64> {
        model::validate_mailbox(mailbox)?;
        self.blob_quota.ensure_available(raw.len())?;
        let index = self.index(mailbox);
        let mut uid = self.next_uids.get(mailbox).copied().unwrap_or(1);
        let (name, _) = blobs::next_available(&self.root, mailbox, &mut uid)?;
        blobs::publish(&self.root, &name, raw)?;
        self.blob_quota.record(raw.len())?;
        let mut file = OpenOptions::new().create(true).append(true).open(index)?;
        file.write_all(&journal::frame(uid, &name))?;
        file.sync_all()?;
        durability::sync_directory(&self.root.join("mailboxes"))?;
        let next_uid = uid
            .checked_add(1)
            .ok_or_else(|| io::Error::other("uid exhausted"))?;
        self.next_uids.insert(mailbox.to_owned(), next_uid);
        Ok(uid)
    }
    pub fn messages(&self, mailbox: &str) -> io::Result<Vec<Message>> {
        model::validate_mailbox(mailbox)?;
        journal::records(mailbox, &self.index(mailbox))?
            .into_iter()
            .map(|r| {
                let path = self.root.join("blobs").join(&r.name);
                Ok(Message {
                    uid: r.uid,
                    bytes: fs::read(&path)?,
                    path,
                })
            })
            .collect()
    }
    fn index(&self, mailbox: &str) -> PathBuf {
        self.root
            .join("mailboxes")
            .join(format!("{mailbox}.journal"))
    }
}
#[cfg(target_os = "linux")]
fn open_writer_lock(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new().write(true).create(true).open(path)?;
    crate::platform::linux_lock::acquire(&file)?;
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
