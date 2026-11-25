//! Журнал связывает одну queue job с единственным committed DSN report.

use crate::queue::DsnReportKey;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Записывает dedup mapping до глобального commit report transaction.
pub(super) fn append(root: &Path, report_transaction: u64, key: &DsnReportKey) -> io::Result<()> {
    validate_key(key)?;
    let payload = payload(report_transaction, key);
    let checksum = checksum(payload.as_bytes());
    let path = path(root);
    let parent = path
        .parent()
        .ok_or_else(|| invalid("DSN report journal parent is missing"))?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{payload} {checksum:016x}")?;
    file.sync_all()?;
    super::durability::sync_directory(parent)
}

/// Восстанавливает ключи лишь для globally committed report transactions.
pub(super) fn load(
    root: &Path,
    committed: &HashSet<u64>,
) -> io::Result<HashMap<DsnReportKey, u64>> {
    let source = match fs::read_to_string(path(root)) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    let mut result = HashMap::new();
    for line in source.lines() {
        let (report_id, key) = parse(line)?;
        if committed.contains(&report_id) {
            match result.get(&key) {
                Some(previous) if *previous != report_id => {
                    return Err(invalid("duplicate committed DSN report key"));
                }
                Some(_) => {}
                None => {
                    result.insert(key, report_id);
                }
            }
        }
    }
    Ok(result)
}

/// Удаляет только незавершённую последнюю строку перед разбором журнала.
pub(super) fn recover(root: &Path) -> io::Result<()> {
    let path = path(root);
    let source = match fs::read(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    match source.last() {
        Some(b'\n') | None => Ok(()),
        Some(_) => {
            let length = source
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(0, |position| position.saturating_add(1));
            let file = OpenOptions::new().write(true).open(path)?;
            file.set_len(length as u64)?;
            file.sync_all()
        }
    }
}

fn parse(line: &str) -> io::Result<(u64, DsnReportKey)> {
    let mut fields = line.split_whitespace();
    let (Some(report), Some(transaction), Some(ordinal), Some(notice), Some(recipient), Some(sum)) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err(invalid("incomplete DSN report frame"));
    };
    if fields.next().is_some() {
        return Err(invalid("trailing DSN report fields"));
    }
    let report_id = parse_number(report, "DSN report transaction")?;
    let transaction_id = parse_number(transaction, "DSN source transaction")?;
    let original_recipient_indices = match ordinal {
        "-" => Vec::new(),
        value => parse_ordinals(value)?,
    };
    let notice = crate::dsn::Notice::from_journal_tag(notice)
        .ok_or_else(|| invalid("unknown DSN report notice tag"))?;
    let recipient = decode_hex(recipient)?;
    let key = DsnReportKey {
        transaction_id,
        recipient,
        original_recipient_indices,
        notice,
    };
    validate_key(&key)?;
    let expected = u64::from_str_radix(sum, 16).map_err(|_| invalid("bad DSN report checksum"))?;
    if checksum(payload(report_id, &key).as_bytes()) != expected {
        return Err(invalid("DSN report checksum mismatch"));
    }
    Ok((report_id, key))
}

fn payload(report: u64, key: &DsnReportKey) -> String {
    let ordinal = crate::queue::encode_recipient_indices(&key.original_recipient_indices);
    format!(
        "{report} {} {ordinal} {} {}",
        key.transaction_id,
        key.notice.journal_tag(),
        encode_hex(key.recipient.as_bytes())
    )
}

/// Проверяет упорядоченный список исходных RCPT в idempotency journal.
fn parse_ordinals(value: &str) -> io::Result<Vec<usize>> {
    let mut ordinals = Vec::new();
    let mut previous = None;
    for item in value.split(',') {
        let ordinal = usize::try_from(parse_number(item, "DSN recipient ordinal")?)
            .map_err(|_| invalid("DSN recipient ordinal overflow"))?;
        match previous {
            Some(last) if ordinal <= last => {
                return Err(invalid("unordered DSN recipient ordinals"));
            }
            _ => {}
        }
        previous = Some(ordinal);
        ordinals.push(ordinal);
    }
    Ok(ordinals)
}

fn validate_key(key: &DsnReportKey) -> io::Result<()> {
    match (key.recipient.is_empty(), key.recipient.len() > 512) {
        (true, _) | (_, true) => Err(invalid("DSN report recipient length")),
        (false, false) => Ok(()),
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        })
}

fn decode_hex(value: &str) -> io::Result<String> {
    if !value.len().is_multiple_of(2) {
        return Err(invalid("odd DSN report recipient encoding"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().as_chunks::<2>().0 {
        bytes.push((hex_digit(pair[0])? << 4) | hex_digit(pair[1])?);
    }
    String::from_utf8(bytes).map_err(|_| invalid("DSN report recipient is not UTF-8"))
}

fn hex_digit(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(invalid("bad DSN report recipient hex")),
    }
}

fn parse_number(value: &str, field: &'static str) -> io::Result<u64> {
    value.parse().map_err(|_| invalid(field))
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |mut value, byte| {
            value ^= u64::from(*byte);
            value.wrapping_mul(0x100_0000_01b3)
        })
}

fn path(root: &Path) -> PathBuf {
    root.join("journal/dsn-reports.log")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
