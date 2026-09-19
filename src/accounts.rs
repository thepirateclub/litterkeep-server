//! File-backed account credentials. Password hashing stays outside the reactor.
use crate::auth::Credential;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct Accounts {
    root: PathBuf,
}
impl Accounts {
    pub fn open(data: impl AsRef<Path>) -> io::Result<Self> {
        let root = data.as_ref().join("accounts");
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    pub fn create(&self, name: &str, password: &[u8]) -> io::Result<()> {
        valid_name(name)?;
        let credential = Credential::create(password)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path(name))?;
        file.write_all(credential.as_phc().as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()
    }
    pub fn authenticate(&self, name: &str, password: &[u8]) -> bool {
        match valid_name(name).and_then(|_| fs::read_to_string(self.path(name))) {
            Ok(phc) => Credential::from_phc(phc.trim().into())
                .is_ok_and(|credential| credential.verify(password)),
            Err(_) => false,
        }
    }
    /// Сообщает, существует ли допустимый локальный получатель.
    ///
    /// Эта проверка намеренно не раскрывает credential и нужна writer-у до
    /// публикации SMTP-сообщения в mailbox получателя.
    pub fn exists(&self, name: &str) -> bool {
        valid_name(name)
            .map(|()| self.path(name).is_file())
            .unwrap_or(false)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.phc"))
    }
}
fn valid_name(name: &str) -> io::Result<()> {
    match name.is_empty() {
        true => Err(io::Error::new(io::ErrorKind::InvalidInput, "empty account")),
        false => match name
            .bytes()
            .find(|byte| !matches!(byte,b'0'..=b'9'|b'A'..=b'Z'|b'a'..=b'z'|b'_'|b'-'))
        {
            Some(_) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe account",
            )),
            None => Ok(()),
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    #[test]
    fn credential_round_trip() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-accounts-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let accounts = Accounts::open(&root).unwrap();
        accounts.create("bob", b"test-password").unwrap();
        assert!(accounts.authenticate("bob", b"test-password"));
        assert!(!accounts.authenticate("bob", b"wrong"));
        let _ = fs::remove_dir_all(root);
    }
}
