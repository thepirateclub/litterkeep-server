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
        let path = crate::store::file_key::prepare(
            &self.root,
            name,
            crate::store::file_key::FileKind::Credentials,
        )?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(credential.as_phc().as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        let directory = path.parent().unwrap_or(&self.root);
        crate::store::file_key::sync_key_directory(&self.root, directory)
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
        crate::store::file_key::path(
            &self.root,
            name,
            crate::store::file_key::FileKind::Credentials,
        )
    }
}
fn valid_name(name: &str) -> io::Result<()> {
    // Храним точные UTF-8 октеты канонического local-part без нормализации.
    // Полную SMTP mailbox grammar проверяет протокольный слой до маршрута.
    let invalid_name = name.is_empty()
        || name.len() > crate::smtp::MAX_LOCAL_PART_SIZE
        || name.chars().any(char::is_control);
    match invalid_name {
        true => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid account name",
        )),
        false => Ok(()),
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
