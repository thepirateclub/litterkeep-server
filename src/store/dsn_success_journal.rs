//! Журнал pending success notices, видимых только после исходного commit.

use crate::queue::DsnSuccessKey;
use std::{
    collections::{BTreeSet, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RECIPIENTS: usize = 100;

/// Пишет RCPT success intents перед global commit исходной доставки.
pub(super) fn append(root: &Path, transaction_id: u64, indices: &[usize]) -> io::Result<()> {
    let mut unique = HashSet::with_capacity(indices.len());
    for index in indices {
        if *index >= MAX_RECIPIENTS || !unique.insert(*index) {
            return Err(invalid(
                "invalid or duplicate DSN success recipient ordinal",
            ));
        }
    }
    if indices.is_empty() {
        return Ok(());
    }
    let path = journal_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("DSN success journal parent is missing"))?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    let mut projected_size = file.metadata()?.len();
    for index in indices {
        let payload = format!("v1 {transaction_id} {index}");
        let frame = format!("{payload} {:016x}\n", checksum(payload.as_bytes()));
        projected_size = projected_size.saturating_add(frame.len() as u64);
        if projected_size > MAX_JOURNAL_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "DSN success journal reached its configured limit",
            ));
        }
        file.write_all(frame.as_bytes())?;
    }
    file.sync_all()?;
    super::durability::sync_directory(parent)
}

/// Возвращает intents только для исходных транзакций с commit marker.
pub(super) fn load(root: &Path, committed: &HashSet<u64>) -> io::Result<BTreeSet<DsnSuccessKey>> {
    let source = match fs::read_to_string(journal_path(root)) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(error) => return Err(error),
    };
    let mut result = BTreeSet::new();
    for line in source.lines() {
        let (transaction_id, recipient_index) = parse(line)?;
        if committed.contains(&transaction_id) {
            result.insert(DsnSuccessKey {
                transaction_id,
                recipient_index,
            });
        }
    }
    Ok(result)
}

/// Обрезает только неполный последний frame до загрузки индекса.
pub(super) fn recover(root: &Path) -> io::Result<()> {
    let path = journal_path(root);
    let bytes = match fs::read(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if bytes.len() as u64 > MAX_JOURNAL_BYTES {
        return Err(invalid("DSN success journal exceeds its configured limit"));
    }
    match bytes.last() {
        Some(b'\n') | None => Ok(()),
        Some(_) => {
            let length = bytes
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |position| position.saturating_add(1));
            let file = OpenOptions::new().write(true).open(path)?;
            file.set_len(length as u64)?;
            file.sync_all()
        }
    }
}

fn parse(line: &str) -> io::Result<(u64, usize)> {
    let mut fields = line.split_whitespace();
    let (Some(version), Some(transaction), Some(ordinal), Some(sum)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(invalid("incomplete DSN success frame"));
    };
    if version != "v1" || fields.next().is_some() {
        return Err(invalid("invalid DSN success frame fields"));
    }
    let transaction_id = parse_number(transaction)?;
    let recipient_index = usize::try_from(parse_number(ordinal)?)
        .map_err(|_| invalid("DSN success recipient ordinal overflow"))?;
    if recipient_index >= MAX_RECIPIENTS {
        return Err(invalid("DSN success recipient ordinal exceeds limit"));
    }
    let payload = format!("v1 {transaction_id} {recipient_index}");
    let actual = u64::from_str_radix(sum, 16).map_err(|_| invalid("bad DSN success checksum"))?;
    if checksum(payload.as_bytes()) != actual {
        return Err(invalid("DSN success checksum mismatch"));
    }
    Ok((transaction_id, recipient_index))
}

fn parse_number(value: &str) -> io::Result<u64> {
    value
        .parse()
        .map_err(|_| invalid("invalid DSN success number"))
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |mut value, byte| {
            value ^= u64::from(*byte);
            value.wrapping_mul(0x100_0000_01b3)
        })
}

fn journal_path(root: &Path) -> PathBuf {
    root.join("journal/dsn-success.log")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
