//! Small append-only filesystem mailbox store.  Metadata is committed only
//! after the blob has been flushed and atomically published.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    _writer_lock: File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub uid: u64,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

impl Store {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("blobs"))?;
        fs::create_dir_all(root.join("mailboxes"))?;
        let writer_lock = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("writer.lock"))?;
        Ok(Self {
            root,
            _writer_lock: writer_lock,
        })
    }
    pub fn append(&self, mailbox: &str, raw: &[u8]) -> io::Result<u64> {
        if !mailbox
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe mailbox",
            ));
        }
        let index = self
            .root
            .join("mailboxes")
            .join(format!("{mailbox}.journal"));
        let uid = fs::read_to_string(&index)
            .ok()
            .and_then(|s| {
                s.lines()
                    .last()
                    .and_then(|l| l.split_once(' ').and_then(|(u, _)| u.parse().ok()))
            })
            .unwrap_or(0)
            + 1;
        let name = format!("{mailbox}-{uid}.eml");
        let final_path = self.root.join("blobs").join(&name);
        let temp = self.root.join("blobs").join(format!(".{name}.tmp"));
        {
            let mut blob = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            blob.write_all(raw)?;
            blob.sync_all()?;
        }
        fs::rename(&temp, &final_path)?;
        let mut journal = OpenOptions::new().create(true).append(true).open(&index)?;
        writeln!(journal, "{uid} {name}")?;
        journal.sync_all()?;
        Ok(uid)
    }
    pub fn messages(&self, mailbox: &str) -> io::Result<Vec<Message>> {
        let index = self
            .root
            .join("mailboxes")
            .join(format!("{mailbox}.journal"));
        let text = match fs::read_to_string(index) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        text.lines()
            .map(|line| {
                let (uid, name) = line
                    .split_once(' ')
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "bad journal"))?;
                let path = self.root.join("blobs").join(name);
                Ok(Message {
                    uid: uid
                        .parse()
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad uid"))?,
                    bytes: fs::read(&path)?,
                    path,
                })
            })
            .collect()
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root.join("writer.lock"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "litterkeep-store-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
    #[test]
    fn committed_message_survives_reopen() {
        let root = dir();
        let store = Store::open(&root).expect("open");
        assert_eq!(
            store
                .append("bob", b"Subject: x\r\n\r\nbytes")
                .expect("append"),
            1
        );
        drop(store);
        let reopened = Store::open(&root).expect("reopen");
        let messages = reopened.messages("bob").expect("read");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].bytes, b"Subject: x\r\n\r\nbytes");
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }
    #[test]
    fn second_writer_is_rejected() {
        let root = dir();
        let first = Store::open(&root).expect("first");
        assert_eq!(
            Store::open(&root).expect_err("second writer").kind(),
            io::ErrorKind::AlreadyExists
        );
        drop(first);
        fs::remove_dir_all(root).expect("cleanup");
    }
    #[test]
    fn rejects_path_traversal_mailbox_name() {
        let root = dir();
        let store = Store::open(&root).expect("open");
        assert_eq!(
            store
                .append("../other", b"x")
                .expect_err("unsafe mailbox")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
