//! Межпроцессная блокировка единственного писателя Store.

#[cfg(not(target_os = "linux"))]
use super::Store;
use super::{File, OpenOptions, Path};
use std::io;

/// Открывает файл блокировки с платформенным контрактом единственного writer.
#[cfg(target_os = "linux")]
pub(super) fn open_writer_lock(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    if let Err(error) = crate::platform::linux_lock::acquire(&file) {
        return match error.kind() {
            io::ErrorKind::WouldBlock => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "store writer is already active",
            )),
            _ => Err(error),
        };
    }
    Ok(file)
}

/// Удаляет маркер, использовавшийся для блокировки на платформах без flock.
#[cfg(not(target_os = "linux"))]
impl Drop for Store {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.root.join("writer.lock"));
    }
}

/// Создаёт эксклюзивный lock marker на платформах без поддержанного flock.
#[cfg(not(target_os = "linux"))]
pub(super) fn open_writer_lock(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}
