//! Явная версия дискового формата до открытия writer-а и recovery.
//!
//! Сервер не пытается угадать или молча переписать старый journal. Миграция
//! должна быть отдельной offline-командой, которой в текущем checkpoint нет.
use super::durability;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const FORMAT: &[u8] = b"litterkeep-store-v2\n";

/// Проверяет существующий marker либо долговечно создаёт его в пустом store.
pub(super) fn ensure(root: &Path) -> io::Result<()> {
    let marker = root.join("format-version");
    match fs::read(&marker) {
        Ok(bytes) if bytes == FORMAT => return Ok(()),
        Ok(_) => return Err(invalid("unsupported store format")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    if contains_mail_data(root)? {
        return Err(invalid("unversioned store requires offline migration"));
    }
    let temporary = root.join(".format-version.tmp");
    match fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(FORMAT)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, marker)?;
    durability::sync_directory(root)
}

/// Учётные записи могут существовать до первого запуска Store и не являются mail data.
fn contains_mail_data(root: &Path) -> io::Result<bool> {
    for name in ["blobs", "mailboxes", "folders"] {
        let directory = root.join(name);
        match fs::read_dir(directory) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Ok(true);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
