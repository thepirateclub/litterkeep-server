//! Долговечная очередь неприсвоенных IMAP `\\Recent` UID.
//!
//! `\\Recent` не является постоянным message flag. Она принадлежит выбранной
//! IMAP-сессии, однако доставка до первого SELECT должна пережить restart.
//! Этот журнал хранит только ещё не назначенные UID. Добавление одной delivery
//! transaction применимо лишь после её общего commit, а claim фиксируется
//! отдельно до выдачи SELECT success.

use super::{commit_log, durability, model::Record};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const MAGIC: [u8; 4] = *b"LKRC";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xa9e1_0c7d;
const ADD: u8 = 1;
const CLAIM: u8 = 2;
const DROP: u8 = 3;
const MAX_NAME: usize = 256;

/// Загружает применимые frames и обрезает только недописанный tail.
pub(super) fn load(
    root: &Path,
    committed: &commit_log::Committed,
) -> io::Result<HashMap<String, BTreeSet<u64>>> {
    let path = path(root);
    let bytes = match fs::read(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    let (frames, valid) = decode_all(&bytes)?;
    if valid < bytes.len() {
        let file = OpenOptions::new().write(true).open(path)?;
        file.set_len(valid as u64)?;
        file.sync_all()?;
    }
    let mut values = HashMap::new();
    for frame in frames {
        if frame.applies(committed) {
            apply(&mut values, frame);
        }
    }
    Ok(values)
}

/// Журналирует UID, появившиеся в новых mailbox links до global commit.
pub(super) fn add(root: &Path, transaction_id: u64, links: &[(String, Record)]) -> io::Result<()> {
    let entries = links
        .iter()
        .map(|(mailbox, record)| (mailbox.as_str(), record.uid))
        .collect::<Vec<_>>();
    append(root, Frame::transaction(ADD, transaction_id, entries))
}

/// Журналирует source UID, удалённые transaction-операцией MOVE.
pub(super) fn drop_transaction(
    root: &Path,
    transaction_id: u64,
    mailbox: &str,
    uids: &[u64],
) -> io::Result<()> {
    let entries = uids.iter().map(|uid| (mailbox, *uid)).collect::<Vec<_>>();
    append(root, Frame::transaction(DROP, transaction_id, entries))
}

/// Удаляет назначенные UID до отправки успешного SELECT ответа.
pub(super) fn claim(root: &Path, mailbox: &str, uids: &[u64]) -> io::Result<()> {
    let entries = uids.iter().map(|uid| (mailbox, *uid)).collect::<Vec<_>>();
    append(root, Frame::immediate(CLAIM, entries))
}

/// Удаляет UID после обычного durable EXPUNGE.
pub(super) fn drop_immediate(root: &Path, mailbox: &str, uids: &[u64]) -> io::Result<()> {
    let entries = uids.iter().map(|uid| (mailbox, *uid)).collect::<Vec<_>>();
    append(root, Frame::immediate(DROP, entries))
}

fn path(root: &Path) -> std::path::PathBuf {
    root.join("journal/recent.log")
}

fn append(root: &Path, frame: Frame<'_>) -> io::Result<()> {
    let bytes = encode(&frame)?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path(root))
        .and_then(|mut file| file.write_all(&bytes).and_then(|()| file.sync_all()))?;
    durability::sync_directory(&root.join("journal"))
}

struct Frame<'a> {
    operation: u8,
    transaction_id: Option<u64>,
    entries: Vec<(&'a str, u64)>,
}

impl<'a> Frame<'a> {
    fn transaction(operation: u8, transaction_id: u64, entries: Vec<(&'a str, u64)>) -> Self {
        Self {
            operation,
            transaction_id: Some(transaction_id),
            entries,
        }
    }

    fn immediate(operation: u8, entries: Vec<(&'a str, u64)>) -> Self {
        Self {
            operation,
            transaction_id: None,
            entries,
        }
    }
}

struct DecodedFrame {
    operation: u8,
    transaction_id: Option<u64>,
    entries: Vec<(String, u64)>,
}

impl DecodedFrame {
    fn applies(&self, committed: &commit_log::Committed) -> bool {
        self.transaction_id
            .is_none_or(|id| committed.ids.contains(&id))
    }
}

fn apply(values: &mut HashMap<String, BTreeSet<u64>>, frame: DecodedFrame) {
    for (mailbox, uid) in frame.entries {
        match frame.operation {
            ADD => {
                values.entry(mailbox).or_default().insert(uid);
            }
            CLAIM | DROP => {
                if let Some(uids) = values.get_mut(&mailbox) {
                    uids.remove(&uid);
                }
            }
            _ => unreachable!("decoded operation is checked"),
        }
    }
}

fn encode(frame: &Frame<'_>) -> io::Result<Vec<u8>> {
    let count = u32::try_from(frame.entries.len()).map_err(|_| invalid("too many recent UIDs"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_be_bytes());
    bytes.push(frame.operation);
    match frame.transaction_id {
        Some(id) => bytes.extend_from_slice(&id.to_be_bytes()),
        None => bytes.extend_from_slice(&0_u64.to_be_bytes()),
    }
    bytes.extend_from_slice(&count.to_be_bytes());
    for (mailbox, uid) in &frame.entries {
        let name = mailbox.as_bytes();
        let length = u16::try_from(name.len()).map_err(|_| invalid("recent mailbox too long"))?;
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&uid.to_be_bytes());
    }
    bytes.extend_from_slice(&checksum(&bytes).to_be_bytes());
    bytes.extend_from_slice(&COMMIT.to_be_bytes());
    Ok(bytes)
}

fn decode_all(bytes: &[u8]) -> io::Result<(Vec<DecodedFrame>, usize)> {
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let decoded = match decode_at(bytes, offset) {
            Ok(value) => value,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => None,
            Err(error) => return Err(error),
        };
        let Some((frame, next)) = decoded else {
            break;
        };
        result.push(frame);
        offset = next;
    }
    Ok((result, offset))
}

fn decode_at(bytes: &[u8], offset: usize) -> io::Result<Option<(DecodedFrame, usize)>> {
    let mut decoder = Decoder::new(bytes, offset);
    let Some(magic) = decoder.take::<4>()? else {
        return Ok(None);
    };
    if magic != MAGIC || u16::from_be_bytes(decoder.required::<2>()?) != VERSION {
        return Err(invalid("bad recent frame"));
    }
    let operation = u8::from_be_bytes(decoder.required::<1>()?);
    if !matches!(operation, ADD | CLAIM | DROP) {
        return Err(invalid("bad recent operation"));
    }
    let raw_transaction = u64::from_be_bytes(decoder.required::<8>()?);
    let transaction_id = match raw_transaction {
        0 => None,
        value => Some(value),
    };
    if matches!((operation, transaction_id), (ADD, None)) {
        return Err(invalid("recent add without transaction"));
    }
    let count = u32::from_be_bytes(decoder.required::<4>()?) as usize;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let length = usize::from(u16::from_be_bytes(decoder.required::<2>()?));
        if length > MAX_NAME {
            return Err(invalid("recent mailbox too long"));
        }
        let name = std::str::from_utf8(decoder.required_slice(length)?)
            .map_err(|_| invalid("recent non utf8 mailbox"))?;
        super::model::validate_mailbox(name)?;
        let uid = u64::from_be_bytes(decoder.required::<8>()?);
        if uid == 0 {
            return Err(invalid("recent zero UID"));
        }
        entries.push((name.to_owned(), uid));
    }
    let checksum_at = decoder.position();
    let expected = u64::from_be_bytes(decoder.required::<8>()?);
    if u32::from_be_bytes(decoder.required::<4>()?) != COMMIT {
        return Err(invalid("recent commit marker"));
    }
    let protected = bytes
        .get(offset..checksum_at)
        .ok_or_else(|| invalid("recent frame"))?;
    if checksum(protected) != expected {
        return Err(invalid("recent checksum"));
    }
    Ok(Some((
        DecodedFrame {
            operation,
            transaction_id,
            entries,
        },
        decoder.position(),
    )))
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }
    fn position(&self) -> usize {
        self.offset
    }
    fn take<const SIZE: usize>(&mut self) -> io::Result<Option<[u8; SIZE]>> {
        self.take_slice(SIZE)
            .map(|value| value.and_then(|slice| slice.try_into().ok()))
    }
    fn required<const SIZE: usize>(&mut self) -> io::Result<[u8; SIZE]> {
        self.take::<SIZE>()?.ok_or_else(incomplete)
    }
    fn required_slice(&mut self, length: usize) -> io::Result<&'a [u8]> {
        self.take_slice(length)?.ok_or_else(incomplete)
    }
    fn take_slice(&mut self, length: usize) -> io::Result<Option<&'a [u8]>> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| invalid("recent offset"))?;
        let result = self.bytes.get(self.offset..end);
        if result.is_some() {
            self.offset = end;
        }
        Ok(result)
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x100_0000_01b3);
    }
    value
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn incomplete() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete recent frame")
}
