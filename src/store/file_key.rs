//! Безопасное отображение логических mailbox-ключей на имена файлов.
//!
//! Старые ASCII ключи сохраняют прежние пути. Остальные ключи хранятся в
//! отдельном каталоге с hex-представлением UTF-8, поэтому символы протокола
//! никогда не становятся компонентами системного пути.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[path = "file_key_codec.rs"]
mod codec;
#[path = "file_key_scan.rs"]
mod scan;

const ENCODED_DIRECTORY: &str = "utf8";
const HEX_DIRECTORY_CHUNK: usize = 120;
const MAX_ENCODED_DIRECTORY_DEPTH: usize = 4;

/// Разделяет семантические виды mailbox-файлов до работы с дисковым путём.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileKind {
    Journal,
    Flags,
    Expunges,
    Credentials,
    FolderCatalog,
    Subscriptions,
    SieveState,
    SieveTemporary,
}

impl FileKind {
    fn extension(self) -> &'static str {
        match self {
            Self::Journal => "journal",
            Self::Flags => "flags",
            Self::Expunges => "expunges",
            Self::Credentials => "phc",
            Self::FolderCatalog | Self::Subscriptions => "journal",
            Self::SieveState => "state",
            Self::SieveTemporary => "tmp",
        }
    }

    fn hidden(self) -> bool {
        matches!(self, Self::SieveTemporary)
    }
}

/// Возвращает устойчивый путь файла для ключа и расширения.
pub(crate) fn path(root: &Path, key: &str, kind: FileKind) -> PathBuf {
    let extension = kind.extension();
    match legacy_key(key) {
        true => legacy_path(root, key, kind, extension),
        false => encoded_path(root, key, kind, extension),
    }
}

fn legacy_path(root: &Path, key: &str, kind: FileKind, extension: &str) -> PathBuf {
    match kind.hidden() {
        true => root.join(format!(".{key}.{extension}")),
        false => root.join(format!("{key}.{extension}")),
    }
}

fn encoded_path(root: &Path, key: &str, kind: FileKind, extension: &str) -> PathBuf {
    let mut directory = root.join(ENCODED_DIRECTORY);
    let mut remaining = codec::encode_hex(key.as_bytes());
    while remaining.len() > HEX_DIRECTORY_CHUNK {
        let Some((chunk, tail)) = remaining.split_at_checked(HEX_DIRECTORY_CHUNK) else {
            break;
        };
        directory.push(chunk);
        remaining = tail.to_owned();
    }
    let stem = match kind.hidden() {
        true => format!(".{remaining}"),
        false => remaining,
    };
    directory.join(format!("{stem}.{extension}"))
}

/// Создаёт кодированное пространство имён и сохраняет его запись в родителе.
pub(crate) fn prepare(root: &Path, key: &str, kind: FileKind) -> io::Result<PathBuf> {
    let path = path(root, key, kind);
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing key parent",
        ));
    };
    if parent != root {
        ensure_path_directories(root, parent)?;
    }
    Ok(path)
}

fn ensure_path_directories(root: &Path, target: &Path) -> io::Result<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| invalid("encoded key path escaped its namespace"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        let Some(parent) = current.parent() else {
            return Err(invalid("encoded key directory has no parent"));
        };
        ensure_directory(parent, &current)?;
    }
    Ok(())
}

fn ensure_directory(parent: &Path, directory: &Path) -> io::Result<()> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(invalid("encoded mailbox namespace is not a directory")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(directory) {
            Ok(()) => sync_key_directory(parent, directory),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                match fs::symlink_metadata(directory) {
                    Ok(metadata) if metadata.is_dir() => Ok(()),
                    Ok(_) => Err(invalid("encoded mailbox namespace is not a directory")),
                    Err(metadata_error) => Err(metadata_error),
                }
            }
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    }
}

/// Читает файлы обоих пространств имён без изменения старых данных.
pub(crate) fn entries(root: &Path, kind: FileKind) -> io::Result<Vec<(String, PathBuf)>> {
    scan::entries(root, kind)
}

/// Синхронизирует все родительские каталоги до корня namespace.
pub(crate) fn sync_key_directory(root: &Path, path: &Path) -> io::Result<()> {
    let mut current = path;
    loop {
        super::durability::sync_directory(current)?;
        if current == root {
            return Ok(());
        }
        current = current
            .parent()
            .ok_or_else(|| invalid("encoded key path escaped its namespace"))?;
    }
}

fn legacy_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
#[path = "file_key_tests.rs"]
mod tests;
