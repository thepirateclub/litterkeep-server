//! Ограниченный budget диска для неизменяемых message blobs.
//!
//! При старте выполняется единственный линейный обход `O(n)`. После recovery
//! единственный writer проверяет и обновляет счётчик за `O(1)`, поэтому quota
//! не создаёт повторного сканирования каталога для каждого SMTP commit.
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub(super) struct Quota {
    limit: u64,
    used: u64,
}

impl Quota {
    /// Восстанавливает фактическое использование, включая orphan и temp blobs.
    pub(super) fn load(directory: &Path, limit: u64) -> io::Result<Self> {
        let mut used = 0_u64;
        for entry in fs::read_dir(directory)? {
            let metadata = entry?.metadata()?;
            if metadata.is_file() {
                used = used
                    .checked_add(metadata.len())
                    .ok_or_else(|| io::Error::other("blob usage overflow"))?;
            }
        }
        if used > limit {
            return Err(storage_full("existing blobs exceed configured quota"));
        }
        Ok(Self { limit, used })
    }

    /// Отказывает до создания temp-файла, если message не помещается в budget.
    pub(super) fn ensure_available(&self, bytes: usize) -> io::Result<()> {
        let bytes = u64::try_from(bytes).map_err(|_| storage_full("message size overflow"))?;
        match self.used.checked_add(bytes) {
            Some(total) if total <= self.limit => Ok(()),
            Some(_) | None => Err(storage_full("blob quota exceeded")),
        }
    }

    /// Учитывает только уже опубликованное на диске имя blob.
    pub(super) fn record(&mut self, bytes: usize) -> io::Result<()> {
        let bytes = u64::try_from(bytes).map_err(|_| storage_full("message size overflow"))?;
        self.used = self
            .used
            .checked_add(bytes)
            .ok_or_else(|| storage_full("blob usage overflow"))?;
        Ok(())
    }
}

fn storage_full(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::StorageFull, message)
}
