//! Долговечные SCRAM sidecar записи с атомарной публикацией.
use crate::sasl::scram_sha1_record::ScramSha1Record;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Создаёт уникальную SCRAM-запись рядом с основным account credential.
pub(super) fn create(root: &Path, name: &str, password: &[u8]) -> io::Result<()> {
    let record = ScramSha1Record::create(password)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid SCRAM password"))?;
    let path = crate::store::file_key::prepare(
        root,
        name,
        crate::store::file_key::FileKind::ScramCredentials,
    )?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(record.encode().as_bytes())?;
    file.sync_all()?;
    drop(file);
    let directory = path.parent().unwrap_or(root);
    crate::store::file_key::sync_key_directory(root, directory)
}

/// Удаляет SCRAM sidecar при откате незавершённого account creation.
pub(super) fn remove(root: &Path, name: &str) -> io::Result<()> {
    let path = crate::store::file_key::path(
        root,
        name,
        crate::store::file_key::FileKind::ScramCredentials,
    );
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Загружает verifier только из полностью валидной versioned записи.
pub(super) fn load(root: &Path, name: &str) -> io::Result<Option<ScramSha1Record>> {
    let path = crate::store::file_key::path(
        root,
        name,
        crate::store::file_key::FileKind::ScramCredentials,
    );
    match fs::read_to_string(path) {
        Ok(encoded) => ScramSha1Record::parse(&encoded)
            .map(Some)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid SCRAM record")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
