//! Транзакционный журнал доступных пользователю runtime-ошибок Sieve.
//!
//! Frame записывается до общей commit-точки доставки. При чтении учитываются
//! только committed transaction ID, поэтому оборванная доставка не публикует
//! ложную диагностику, а успешная восстанавливает её вместе с письмом.

use crate::sieve::ExecutionError;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_ACCOUNT_BYTES: usize = 256;
const MAX_JOURNAL_BYTES: u64 = 1024 * 1024;

/// Версия frame после единственной string-to-enum границы codec-а.
enum FormatVersion {
    V1,
}

impl FormatVersion {
    fn code(&self) -> &'static str {
        match self {
            Self::V1 => "1",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "1" => Some(Self::V1),
            _ => None,
        }
    }
}

/// Диагностика, привязанная к версии скрипта и committed delivery transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SieveDiagnostic {
    pub transaction_id: u64,
    pub account: String,
    pub script_version: u64,
    pub error: ExecutionError,
}

/// Ещё не опубликованная запись, которой Store назначит transaction ID.
pub(super) struct PendingDiagnostic {
    pub account: String,
    pub script_version: u64,
    pub error: ExecutionError,
}

/// Долговечно добавляет bounded-набор записей до global transaction commit.
pub(super) fn append(
    root: &Path,
    transaction_id: u64,
    diagnostics: &[PendingDiagnostic],
) -> io::Result<()> {
    if diagnostics.is_empty() {
        return Ok(());
    }
    let mut bytes = Vec::new();
    for diagnostic in diagnostics {
        validate_account(&diagnostic.account)?;
        let line = format!(
            "{}\t{transaction_id}\t{}\t{}\t{}\n",
            FormatVersion::V1.code(),
            diagnostic.script_version,
            diagnostic.error.code(),
            encode_hex(diagnostic.account.as_bytes())
        );
        bytes.extend_from_slice(line.as_bytes());
    }
    let existing = match fs::metadata(path(root)) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error),
    };
    match existing.checked_add(bytes.len() as u64) {
        Some(size) if size <= MAX_JOURNAL_BYTES => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "sieve diagnostic quota exceeded",
            ));
        }
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path(root))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    super::durability::sync_directory(&root.join("journal"))
}

/// Читает только записи завершённых транзакций выбранного владельца.
pub(super) fn load(
    root: &Path,
    committed: &HashSet<u64>,
    account: &str,
) -> io::Result<Vec<SieveDiagnostic>> {
    validate_account(account)?;
    Ok(read_complete(root)?
        .into_iter()
        .filter(|diagnostic| {
            matches!(
                (
                    committed.contains(&diagnostic.transaction_id),
                    diagnostic.account.as_str() == account,
                ),
                (true, true)
            )
        })
        .collect())
}

/// Проверяет complete frames и удаляет оборванный tail до выдачи Store writer-а.
pub(super) fn recover(root: &Path) -> io::Result<()> {
    read_complete(root).map(drop)
}

fn read_complete(root: &Path) -> io::Result<Vec<SieveDiagnostic>> {
    let bytes = match fs::read(path(root)) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let valid = complete_length(&bytes);
    if valid < bytes.len() {
        let file = OpenOptions::new().write(true).open(path(root))?;
        file.set_len(valid as u64)?;
        file.sync_all()?;
    }
    let text = std::str::from_utf8(bytes.get(..valid).unwrap_or_default())
        .map_err(|_| invalid("non-utf8 sieve diagnostic journal"))?;
    let mut result = Vec::new();
    for line in text.lines() {
        result.push(decode(line)?);
    }
    Ok(result)
}

fn decode(line: &str) -> io::Result<SieveDiagnostic> {
    let mut fields = line.split('\t');
    let version = fields
        .next()
        .and_then(FormatVersion::parse)
        .ok_or_else(|| invalid("bad diagnostic frame"))?;
    let transaction_id = number(fields.next(), "bad diagnostic transaction")?;
    let script_version = number(fields.next(), "bad diagnostic script version")?;
    let error = fields
        .next()
        .and_then(ExecutionError::from_code)
        .ok_or_else(|| invalid("bad diagnostic error code"))?;
    let account = fields
        .next()
        .ok_or_else(|| invalid("bad diagnostic account"))
        .and_then(decode_hex)?;
    match (version, fields.next()) {
        (FormatVersion::V1, None) => {}
        _ => return Err(invalid("bad diagnostic frame")),
    }
    validate_account(&account)?;
    Ok(SieveDiagnostic {
        transaction_id,
        account,
        script_version,
        error,
    })
}

fn number(value: Option<&str>, message: &'static str) -> io::Result<u64> {
    value
        .ok_or_else(|| invalid(message))?
        .parse()
        .map_err(|_| invalid(message))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("String formatting cannot fail");
    }
    encoded
}

fn decode_hex(value: &str) -> io::Result<String> {
    let mut chunks = value.as_bytes().chunks_exact(2);
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in &mut chunks {
        let digits = std::str::from_utf8(pair).map_err(|_| invalid("bad diagnostic hex"))?;
        decoded.push(u8::from_str_radix(digits, 16).map_err(|_| invalid("bad diagnostic hex"))?);
    }
    if !chunks.remainder().is_empty() {
        return Err(invalid("bad diagnostic hex"));
    }
    String::from_utf8(decoded).map_err(|_| invalid("non-utf8 diagnostic account"))
}

fn complete_length(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rposition(|value| *value == b'\n')
        .and_then(|position| position.checked_add(1))
        .unwrap_or(0)
}

fn validate_account(account: &str) -> io::Result<()> {
    match (account.is_empty(), account.len() > MAX_ACCOUNT_BYTES) {
        (false, false) => Ok(()),
        _ => Err(invalid("invalid diagnostic account")),
    }
}

fn path(root: &Path) -> PathBuf {
    root.join("journal/sieve-errors.log")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
