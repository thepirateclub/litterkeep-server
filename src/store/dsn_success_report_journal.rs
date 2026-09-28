//! Durable mapping между SUCCESS intent и committed positive report.

use crate::queue::DsnSuccessReportKey;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Записывает mapping до commit отчёта в общей Store-транзакции.
pub(super) fn append(
    root: &Path,
    report_transaction_id: u64,
    key: DsnSuccessReportKey,
) -> io::Result<()> {
    let payload = format!(
        "v1 {report_transaction_id} {} {}",
        key.source_transaction_id, key.recipient_index
    );
    let frame = format!("{payload} {:016x}\n", checksum(payload.as_bytes()));
    let path = journal_path(root);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("success report journal parent is missing"))?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(frame.as_bytes())?;
    file.sync_all()?;
    super::durability::sync_directory(parent)
}

/// Восстанавливает ключи только для глобально committed report transaction.
pub(super) fn load(
    root: &Path,
    committed: &HashSet<u64>,
) -> io::Result<HashMap<DsnSuccessReportKey, u64>> {
    let source = match fs::read_to_string(journal_path(root)) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    let mut reports = HashMap::new();
    for line in source.lines() {
        let (report_id, key) = parse(line)?;
        if !committed.contains(&report_id) {
            continue;
        }
        match reports.get(&key) {
            Some(previous) if *previous != report_id => {
                return Err(invalid("duplicate committed success report key"));
            }
            Some(_) => {}
            None => {
                reports.insert(key, report_id);
            }
        }
    }
    Ok(reports)
}

/// Удаляет только неполную последнюю строку, сохраняя полные frames.
pub(super) fn recover(root: &Path) -> io::Result<()> {
    let path = journal_path(root);
    let bytes = match fs::read(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
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

fn parse(line: &str) -> io::Result<(u64, DsnSuccessReportKey)> {
    let mut fields = line.split_whitespace();
    let (Some(version), Some(report), Some(source), Some(ordinal), Some(sum)) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(invalid("incomplete success report frame"));
    };
    if version != "v1" || fields.next().is_some() {
        return Err(invalid("invalid success report frame fields"));
    }
    let report_id = parse_number(report)?;
    let source_transaction_id = parse_number(source)?;
    let recipient_index = usize::try_from(parse_number(ordinal)?)
        .map_err(|_| invalid("success recipient ordinal overflow"))?;
    if recipient_index >= 100 {
        return Err(invalid("success recipient ordinal exceeds limit"));
    }
    let key = DsnSuccessReportKey {
        source_transaction_id,
        recipient_index,
    };
    let payload = format!("v1 {report_id} {source_transaction_id} {recipient_index}");
    let actual = u64::from_str_radix(sum, 16).map_err(|_| invalid("bad success checksum"))?;
    if checksum(payload.as_bytes()) != actual {
        return Err(invalid("success report checksum mismatch"));
    }
    Ok((report_id, key))
}

fn parse_number(value: &str) -> io::Result<u64> {
    value
        .parse()
        .map_err(|_| invalid("invalid success report number"))
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
    root.join("journal/dsn-success-reports.log")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
