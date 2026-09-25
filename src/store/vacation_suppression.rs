//! Журнал committed идентичностей Sieve vacation-ответов.
//!
//! Записи пишутся до общей commit-точки Store и становятся действующими
//! только если transaction ID присутствует в transactions.log.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_ACCOUNT: usize = 256;
const MAX_SENDER: usize = 320;
const MAX_IDENTITY: usize = 256;
const MAX_JOURNAL_BYTES: u64 = 8 * 1024 * 1024;

/// Структурный ключ устраняет неоднозначность конкатенации полей.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct VacationKey {
    pub account: String,
    pub sender: String,
    pub identity: String,
}

/// Claim добавляется только вместе с generated payload и outbound job.
#[derive(Clone, Debug)]
pub(super) struct PendingClaim {
    pub key: VacationKey,
    pub expires_at: u64,
}

/// Записывает bounded набор claims перед глобальным commit Store.
pub(super) fn append(
    root: &Path,
    transaction_id: u64,
    claim: Option<&PendingClaim>,
) -> io::Result<()> {
    let Some(claim) = claim else {
        return Ok(());
    };
    validate(&claim.key)?;
    let line = format!(
        "1\t{transaction_id}\t{}\t{}\t{}\t{}\n",
        claim.expires_at,
        encode_hex(claim.key.account.as_bytes()),
        encode_hex(claim.key.sender.as_bytes()),
        encode_hex(claim.key.identity.as_bytes())
    );
    let existing = match fs::metadata(path(root)) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error),
    };
    match existing.checked_add(line.len() as u64) {
        Some(size) if size <= MAX_JOURNAL_BYTES => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "vacation history full",
            ));
        }
    }
    let mut journal = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path(root))?;
    journal.write_all(line.as_bytes())?;
    journal.sync_all()?;
    super::durability::sync_directory(&root.join("journal"))
}

/// Загружает только записи committed транзакций, более поздняя запись побеждает.
pub(super) fn load(root: &Path, committed: &HashSet<u64>) -> io::Result<HashMap<VacationKey, u64>> {
    let mut history = HashMap::new();
    for record in read_complete(root)? {
        if committed.contains(&record.transaction_id) {
            history.insert(record.key, record.expires_at);
        }
    }
    Ok(history)
}

/// Восстанавливает журнал, отрезая только неполную последнюю строку.
pub(super) fn recover(root: &Path) -> io::Result<()> {
    read_complete(root).map(drop)
}

struct Record {
    transaction_id: u64,
    expires_at: u64,
    key: VacationKey,
}

fn read_complete(root: &Path) -> io::Result<Vec<Record>> {
    let bytes = match fs::read(path(root)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let valid = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .and_then(|position| position.checked_add(1))
        .unwrap_or(0);
    if valid < bytes.len() {
        let file = OpenOptions::new().write(true).open(path(root))?;
        file.set_len(valid as u64)?;
        file.sync_all()?;
    }
    let text = std::str::from_utf8(bytes.get(..valid).unwrap_or_default())
        .map_err(|_| invalid("non-utf8 vacation journal"))?;
    text.lines().map(decode).collect()
}

fn decode(line: &str) -> io::Result<Record> {
    let mut fields = line.split('\t');
    match fields.next() {
        Some("1") => {}
        _ => return Err(invalid("unsupported vacation journal version")),
    }
    let transaction_id = number(fields.next())?;
    let expires_at = number(fields.next())?;
    let account = decode_hex(fields.next())?;
    let sender = decode_hex(fields.next())?;
    let identity = decode_hex(fields.next())?;
    if fields.next().is_some() {
        return Err(invalid("extra vacation journal field"));
    }
    let key = VacationKey {
        account,
        sender,
        identity,
    };
    validate(&key)?;
    Ok(Record {
        transaction_id,
        expires_at,
        key,
    })
}

pub(super) fn validate(key: &VacationKey) -> io::Result<()> {
    match (
        key.account.is_empty() || key.account.len() > MAX_ACCOUNT,
        key.sender.is_empty() || key.sender.len() > MAX_SENDER,
        key.identity.is_empty() || key.identity.len() > MAX_IDENTITY,
    ) {
        (false, false, false) => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid vacation identity",
        )),
    }
}

fn number(value: Option<&str>) -> io::Result<u64> {
    value
        .ok_or_else(|| invalid("missing vacation number"))?
        .parse()
        .map_err(|_| invalid("invalid vacation number"))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("String formatting cannot fail");
    }
    output
}

fn decode_hex(value: Option<&str>) -> io::Result<String> {
    let value = value.ok_or_else(|| invalid("missing vacation field"))?;
    let mut chunks = value.as_bytes().chunks_exact(2);
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in &mut chunks {
        let digits = std::str::from_utf8(pair).map_err(|_| invalid("invalid vacation hex"))?;
        decoded.push(u8::from_str_radix(digits, 16).map_err(|_| invalid("invalid vacation hex"))?);
    }
    if !chunks.remainder().is_empty() {
        return Err(invalid("odd vacation hex length"));
    }
    String::from_utf8(decoded).map_err(|_| invalid("non-utf8 vacation field"))
}

fn path(root: &Path) -> PathBuf {
    root.join("journal/vacation-history.log")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
