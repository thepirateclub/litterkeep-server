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
        if self.exists(name) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "account credential already exists",
            ));
        }
        let credential = Credential::create(password)?;
        let path = crate::store::file_key::prepare(
            &self.root,
            name,
            crate::store::file_key::FileKind::SaslPrepCredentials,
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
        if valid_name(name).is_err() {
            return false;
        }
        let normalized_path = self.normalized_path(name);
        match fs::read_to_string(normalized_path) {
            Ok(phc) => verify_credential(&phc, password, CredentialFormat::SaslPrep),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.authenticate_legacy(name, password)
            }
            Err(_) => false,
        }
    }
    /// Сообщает, существует ли допустимый локальный получатель.
    ///
    /// Эта проверка намеренно не раскрывает credential и нужна writer-у до
    /// публикации SMTP-сообщения в mailbox получателя.
    pub fn exists(&self, name: &str) -> bool {
        if valid_name(name).is_err() {
            return false;
        }
        match (
            self.path(name).is_file(),
            self.normalized_path(name).is_file(),
        ) {
            (true, _) | (_, true) => true,
            (false, false) => false,
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        crate::store::file_key::path(
            &self.root,
            name,
            crate::store::file_key::FileKind::Credentials,
        )
    }

    /// Возвращает отдельный typed path для SASLprep-нормализованных Argon2 записей.
    fn normalized_path(&self, name: &str) -> PathBuf {
        crate::store::file_key::path(
            &self.root,
            name,
            crate::store::file_key::FileKind::SaslPrepCredentials,
        )
    }

    /// Оставляет старые PHC читаемыми, не выдавая их за нормализованные записи.
    fn authenticate_legacy(&self, name: &str, password: &[u8]) -> bool {
        match fs::read_to_string(self.path(name)) {
            Ok(phc) => verify_credential(&phc, password, CredentialFormat::LegacyRaw),
            Err(_) => false,
        }
    }
}

/// Выбирает password semantics по типизированному storage format.
#[derive(Clone, Copy)]
enum CredentialFormat {
    LegacyRaw,
    SaslPrep,
}

/// Выбирает password semantics по типизированному storage format.
fn verify_credential(phc: &str, password: &[u8], format: CredentialFormat) -> bool {
    let credential = match Credential::from_phc(phc.trim().to_owned()) {
        Ok(credential) => credential,
        Err(_) => return false,
    };
    match format {
        CredentialFormat::LegacyRaw => match credential.verify(password) {
            true => true,
            false => credential.verify_legacy_raw(password),
        },
        CredentialFormat::SaslPrep => credential.verify(password),
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
#[path = "accounts_tests.rs"]
mod tests;
