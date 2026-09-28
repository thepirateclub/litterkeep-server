//! Долговечные SCRAM sidecar записи с атомарной публикацией.
use crate::sasl::scram_sha1_record::ScramSha1Record;
use crate::sasl::scram_sha256_record::ScramSha256Record;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// Создаёт уникальную SCRAM-запись рядом с основным account credential.
pub(super) fn create(root: &Path, name: &str, password: &[u8]) -> io::Result<()> {
    let record = ScramSha1Record::create(password)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid SCRAM password"))?;
    let path = credential_path(
        root,
        name,
        crate::store::file_key::FileKind::ScramCredentials,
    )?;
    publish(root, &path, &record.encode())
}

/// Публикует SHA-256 verifier отдельным versioned sidecar того же аккаунта.
pub(super) fn create_sha256(root: &Path, name: &str, password: &[u8]) -> io::Result<()> {
    let record = ScramSha256Record::create(password).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid SCRAM-SHA-256 password",
        )
    })?;
    let path = credential_path(
        root,
        name,
        crate::store::file_key::FileKind::ScramSha256Credentials,
    )?;
    publish(root, &path, &record.encode())
}

fn publish(root: &Path, path: &Path, encoded: &str) -> io::Result<()> {
    let temporary = temporary_path(path)?;
    let preparation = write_record(&temporary, encoded);
    if let Err(error) = preparation {
        return cleanup_after_failure(&temporary, error);
    }
    let publication = fs::hard_link(&temporary, path);
    let cleanup = fs::remove_file(&temporary);
    match (publication, cleanup) {
        (Ok(()), Ok(())) => sync_parent(root, path),
        (Err(error), Ok(())) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup_error)) => Err(io::Error::new(
            error.kind(),
            format!("{error}; SCRAM temporary cleanup failed: {cleanup_error}"),
        )),
    }
}

/// Дополняет существующую учётную запись после успешной проверки её пароля.
pub(super) fn ensure(root: &Path, name: &str, password: &[u8]) -> io::Result<()> {
    match load(root, name)? {
        Some(_) => Ok(()),
        None => create(root, name, password),
    }
}

/// Создаёт SHA-256 sidecar только после успешной аутентификации аккаунта.
pub(super) fn ensure_sha256(root: &Path, name: &str, password: &[u8]) -> io::Result<()> {
    match load_sha256(root, name)? {
        Some(_) => Ok(()),
        None => create_sha256(root, name, password),
    }
}

/// Удаляет SCRAM sidecar при откате незавершённого account creation.
pub(super) fn remove(root: &Path, name: &str) -> io::Result<()> {
    let sha1 = remove_kind(
        root,
        name,
        crate::store::file_key::FileKind::ScramCredentials,
    );
    let sha256 = remove_kind(
        root,
        name,
        crate::store::file_key::FileKind::ScramSha256Credentials,
    );
    match (sha1, sha256) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(other)) => Err(io::Error::new(
            error.kind(),
            format!("{error}; SHA-256 sidecar cleanup failed: {other}"),
        )),
    }
}

fn remove_kind(root: &Path, name: &str, kind: crate::store::file_key::FileKind) -> io::Result<()> {
    let path = crate::store::file_key::path(root, name, kind);
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

/// Загружает и строго проверяет versioned SCRAM-SHA-256 sidecar.
pub(super) fn load_sha256(root: &Path, name: &str) -> io::Result<Option<ScramSha256Record>> {
    let path = crate::store::file_key::path(
        root,
        name,
        crate::store::file_key::FileKind::ScramSha256Credentials,
    );
    match fs::read_to_string(path) {
        Ok(encoded) => ScramSha256Record::parse(&encoded).map(Some).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid SCRAM-SHA-256 record")
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn credential_path(
    root: &Path,
    name: &str,
    kind: crate::store::file_key::FileKind,
) -> io::Result<std::path::PathBuf> {
    crate::store::file_key::prepare(root, name, kind)
}

/// Записывает и синхронизирует полный временный verifier до его публикации.
fn write_record(path: &Path, encoded: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(encoded.as_bytes())?;
    file.sync_all()
}

/// Создаёт непредсказуемый временный путь в каталоге окончательной записи.
fn temporary_path(path: &Path) -> io::Result<std::path::PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing SCRAM parent"))?;
    let salt = argon2::password_hash::try_generate_salt()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let suffix = salt
        .iter()
        .fold(0_u128, |value, byte| (value << 8) | u128::from(*byte));
    Ok(parent.join(format!(".scram-{}-{suffix:032x}.tmp", std::process::id())))
}

/// Удаляет неполный временный файл и сохраняет обе причины при ошибке уборки.
fn cleanup_after_failure(path: &Path, error: io::Error) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Err(error),
        Err(cleanup_error) => Err(io::Error::new(
            error.kind(),
            format!("{error}; SCRAM temporary cleanup failed: {cleanup_error}"),
        )),
    }
}

/// Синхронизирует каталог после атомарной публикации sidecar.
fn sync_parent(root: &Path, path: &Path) -> io::Result<()> {
    crate::store::file_key::sync_key_directory(root, path.parent().unwrap_or(root))
}
