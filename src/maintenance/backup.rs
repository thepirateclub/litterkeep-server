//! Создание согласованной offline-копии всего каталога данных.

use super::{CheckReport, check};
use crate::store::Store;
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// Результат копирования и последующей проверки восстановленного Store.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupReport {
    pub files: usize,
    pub bytes: u64,
    pub check: CheckReport,
}

/// Тип корневой записи, которая принадлежит только живому процессу.
enum RootEntry {
    Persistent,
    WriterLock,
    RuntimeControl,
}

impl RootEntry {
    fn classify(name: &OsStr) -> Self {
        match name.to_str() {
            Some("writer.lock") => Self::WriterLock,
            Some("runtime.control") => Self::RuntimeControl,
            _ => Self::Persistent,
        }
    }

    fn persistent(&self) -> bool {
        matches!(self, Self::Persistent)
    }
}

/// Удерживает exclusive Store lock до завершения копии и проверяет результат.
pub fn backup(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> io::Result<BackupReport> {
    let store = Store::open(source.as_ref())?;
    store.integrity_report()?;
    let source = fs::canonicalize(source)?;
    let destination = absolute_new_path(destination.as_ref())?;
    if destination.starts_with(&source) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "backup destination is inside source",
        ));
    }
    if destination.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "backup destination already exists",
        ));
    }
    fs::create_dir(&destination)?;
    let copied = copy_tree(&source, &destination);
    drop(store);
    let (files, bytes) = match copied {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
    };
    let check = check(&destination)?;
    Ok(BackupReport {
        files,
        bytes,
        check,
    })
}

/// Разрешает родителя через ОС, не пытаясь canonicalize ещё не созданный путь.
fn absolute_new_path(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "backup destination has no name",
        )
    })?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    Ok(fs::canonicalize(parent)?.join(name))
}

/// Копирует дерево без следования по ссылкам и синхронизирует файлы и каталоги.
fn copy_tree(source: &Path, destination: &Path) -> io::Result<(usize, u64)> {
    let mut pending = VecDeque::from([(source.to_owned(), destination.to_owned(), true)]);
    let mut directories = Vec::new();
    let mut files = 0_usize;
    let mut bytes = 0_u64;
    while let Some((from, to, root)) = pending.pop_front() {
        directories.push(to.clone());
        for entry in fs::read_dir(&from)? {
            let entry = entry?;
            if root && !RootEntry::classify(&entry.file_name()).persistent() {
                continue;
            }
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "symbolic link in data directory",
                ));
            }
            let target = to.join(entry.file_name());
            match (kind.is_dir(), kind.is_file()) {
                (true, false) => {
                    fs::create_dir(&target)?;
                    pending.push_back((entry.path(), target, false));
                }
                (false, true) => {
                    let copied = fs::copy(entry.path(), &target)?;
                    OpenOptions::new().write(true).open(&target)?.sync_all()?;
                    files = files
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("backup file count overflow"))?;
                    bytes = bytes
                        .checked_add(copied)
                        .ok_or_else(|| io::Error::other("backup byte count overflow"))?;
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unsupported entry in data directory",
                    ));
                }
            }
        }
    }
    while let Some(directory) = directories.pop() {
        sync_directory(&directory)?;
    }
    Ok((files, bytes))
}

/// На Linux фиксирует имена в каталоге; Windows служит сборочным профилем.
#[cfg(target_os = "linux")]
fn sync_directory(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(not(target_os = "linux"))]
fn sync_directory(path: &Path) -> io::Result<()> {
    match path.try_exists()? {
        true => Ok(()),
        false => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "backup directory disappeared",
        )),
    }
}
