//! Дисковый ввод-вывод transaction-linked SMTP DSN sidecar.

#[path = "smtp_dsn_cursor.rs"]
mod cursor;
#[path = "smtp_dsn_decode.rs"]
mod decode;
#[path = "smtp_dsn_encode.rs"]
mod encode;
#[path = "smtp_dsn_format.rs"]
mod format;
#[path = "smtp_dsn_payload.rs"]
mod payload;
#[cfg(test)]
#[path = "smtp_dsn_journal_tests.rs"]
mod tests;

use crate::smtp::DsnEnvelope;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Добавляет envelope frame до глобального commit marker сообщения.
pub(super) fn append(root: &Path, transaction_id: u64, envelope: &DsnEnvelope) -> io::Result<()> {
    let frame = encode::frame(transaction_id, envelope)?;
    let path = journal_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("SMTP DSN journal parent is missing"))?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(&frame)?;
    file.sync_all()?;
    super::durability::sync_directory(parent)
}

/// Читает только metadata, связанную с глобально committed transaction ID.
pub(super) fn load(root: &Path, committed: &HashSet<u64>) -> io::Result<HashMap<u64, DsnEnvelope>> {
    let bytes = match fs::read(journal_path(root)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    let frames = decode::scan(&bytes)?;
    let mut envelopes = HashMap::new();
    for (transaction_id, envelope) in frames {
        if committed.contains(&transaction_id) {
            envelopes.insert(transaction_id, envelope);
        }
    }
    Ok(envelopes)
}

/// Обрезает только неполный последний sidecar frame.
pub(super) fn recover(root: &Path) -> io::Result<()> {
    let path = journal_path(root);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let Some(valid_length) = decode::incomplete_tail(&bytes)? else {
        return Ok(());
    };
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(valid_length as u64)?;
    file.sync_all()
}

fn journal_path(root: &Path) -> PathBuf {
    root.join("journal/smtp-dsn.log")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
