//! Filesystem store facade. The writer owns this value exclusively.
#[path = "store/blobs.rs"]
mod blobs;
#[path = "store/journal.rs"]
mod journal;
#[path = "store/model.rs"]
mod model;
#[path = "store/recovery.rs"]
mod recovery;
#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
pub use model::Message;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    _writer_lock: File,
}
impl Store {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("mailboxes"))?;
        let lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("writer.lock"))?;
        let store = Self {
            root,
            _writer_lock: lock,
        };
        recovery::recover(&store.root)?;
        Ok(store)
    }
    pub fn append(&self, mailbox: &str, raw: &[u8]) -> io::Result<u64> {
        model::validate_mailbox(mailbox)?;
        let index = self.index(mailbox);
        let mut uid = journal::records(mailbox, &index)?
            .last()
            .map_or(1, |r| r.uid + 1);
        let (name, _) = blobs::next_available(&self.root, mailbox, &mut uid)?;
        blobs::publish(&self.root, &name, raw)?;
        let mut file = OpenOptions::new().create(true).append(true).open(index)?;
        writeln!(file, "{}", journal::frame(uid, &name))?;
        file.sync_all()?;
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
impl Drop for Store {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root.join("writer.lock"));
    }
}
