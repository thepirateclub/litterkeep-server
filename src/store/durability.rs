//! Долговечность имён файлов после создания и атомарного переименования.
//!
//! `File::sync_all` фиксирует содержимое файла, но на Linux не заменяет fsync
//! каталога, в котором было создано новое имя. Эта операция выполняется только
//! storage worker-ом, поэтому не блокирует сетевой reactor.
use std::io;
use std::path::Path;

/// Синхронизирует metadata каталога на целевой Linux-платформе.
#[cfg(target_os = "linux")]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

/// Windows checkout используется для сборочных тестов, а не runtime-гарантии.
#[cfg(not(target_os = "linux"))]
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    match path.try_exists()? {
        true => Ok(()),
        false => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "directory does not exist",
        )),
    }
}
