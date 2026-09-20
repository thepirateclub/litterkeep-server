use super::{durability, model::blob_name};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Ошибка конкретной низкоуровневой операции публикации blob.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PublishFailure {
    TempCreate,
    ShortWrite,
    FileSync,
    Rename,
    DirectorySync,
}

pub fn next_available(root: &Path, transaction_id: &mut u64) -> io::Result<(String, PathBuf)> {
    loop {
        let name = blob_name(*transaction_id);
        let path = root.join("blobs").join(&name);
        if !path.exists() {
            return Ok((name, path));
        }
        *transaction_id = transaction_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?
    }
}

pub fn publish(root: &Path, name: &str, raw: &[u8]) -> io::Result<()> {
    publish_checked(root, name, raw, |_| Ok(()))
}

#[cfg(test)]
pub(super) fn publish_failing(
    root: &Path,
    name: &str,
    raw: &[u8],
    failure: PublishFailure,
) -> io::Result<()> {
    publish_checked(root, name, raw, |operation| match operation == failure {
        true => Err(io::Error::other(format!("injected {operation:?} failure"))),
        false => Ok(()),
    })
}

/// Выполняет production-путь с тестовым перехватчиком перед системными вызовами.
fn publish_checked<F>(root: &Path, name: &str, raw: &[u8], mut check: F) -> io::Result<()>
where
    F: FnMut(PublishFailure) -> io::Result<()>,
{
    let final_path = root.join("blobs").join(name);
    let temp = root.join("blobs").join(format!(".{name}.tmp"));
    check(PublishFailure::TempCreate)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    if let Err(error) = check(PublishFailure::ShortWrite) {
        let (prefix, _) = raw.split_at(raw.len() / 2);
        file.write_all(prefix)?;
        file.sync_all()?;
        return Err(error);
    }
    file.write_all(raw)?;
    check(PublishFailure::FileSync)?;
    file.sync_all()?;
    drop(file);
    check(PublishFailure::Rename)?;
    fs::rename(temp, final_path)?;
    check(PublishFailure::DirectorySync)?;
    durability::sync_directory(&root.join("blobs"))
}
