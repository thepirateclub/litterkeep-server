//! Durable delivery queue journal.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
#[derive(Debug)]
pub struct Queue {
    path: PathBuf,
}
impl Queue {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            path: root.as_ref().join("delivery.queue"),
        })
    }
    pub fn enqueue(&self, id: &str, recipient: &str) -> io::Result<()> {
        if id.contains(char::is_whitespace) || recipient.contains(char::is_whitespace) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe queue token",
            ));
        };
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(f, "{id} {recipient}")?;
        f.sync_all()
    }
    pub fn pending(&self) -> io::Result<Vec<(String, String)>> {
        match fs::read_to_string(&self.path) {
            Ok(s) => s
                .lines()
                .map(|l| {
                    l.split_once(' ')
                        .map(|(a, b)| (a.into(), b.into()))
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "bad queue record")
                        })
                })
                .collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_survives_reopen() {
        let d = std::env::temp_dir().join("litterkeep-queue-test");
        let q = Queue::open(&d).unwrap();
        q.enqueue("message-1", "bob@test").unwrap();
        assert_eq!(
            Queue::open(&d).unwrap().pending().unwrap(),
            vec![("message-1".into(), "bob@test".into())]
        );
        let _ = fs::remove_dir_all(d);
    }
}
