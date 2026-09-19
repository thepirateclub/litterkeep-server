//! Версионированный бинарный журнал ссылок mailbox на неизменяемые blobs.
//!
//! Числа записываются big-endian. Неполный последний frame отличается от
//! полного frame с неверной checksum или commit marker: recovery обрезает
//! только первый случай и никогда не ищет следующий случайный magic.
use super::model::{Record, blob_name};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

const MAGIC: [u8; 4] = *b"LKJF";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xc0de_c0de;
const MAX_PAYLOAD: usize = 1024;

/// Кодирует один полный frame с последовательностью, transaction id и checksum.
pub(super) fn frame(uid: u64, name: &str) -> Vec<u8> {
    let payload = name.as_bytes();
    let payload_length = u32::try_from(payload.len()).expect("blob name length is bounded");
    let mut bytes = Vec::with_capacity(38 + payload.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_be_bytes());
    bytes.extend_from_slice(&uid.to_be_bytes());
    bytes.extend_from_slice(&uid.to_be_bytes());
    bytes.extend_from_slice(&payload_length.to_be_bytes());
    bytes.extend_from_slice(payload);
    let sum = checksum(&bytes);
    bytes.extend_from_slice(&sum.to_be_bytes());
    bytes.extend_from_slice(&COMMIT.to_be_bytes());
    bytes
}

/// Читает только полностью зафиксированные и проверенные записи.
pub(super) fn records(mailbox: &str, path: &Path) -> io::Result<Vec<Record>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let scan = scan(mailbox, &bytes)?;
    match scan.incomplete_tail {
        Some(_) => Err(invalid("incomplete journal frame")),
        None => Ok(scan.records),
    }
}

/// При запуске обрезает только недописанный финальный frame.
pub(super) fn recover(mailbox: &str, path: &Path) -> io::Result<()> {
    let bytes = fs::read(path)?;
    let scan = scan(mailbox, &bytes)?;
    let Some(valid_length) = scan.incomplete_tail else {
        return Ok(());
    };
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(valid_length as u64)?;
    file.sync_all()
}

struct Scan {
    records: Vec<Record>,
    incomplete_tail: Option<usize>,
}

/// Последовательно декодирует frames и останавливается на первой ошибке.
fn scan(mailbox: &str, bytes: &[u8]) -> io::Result<Scan> {
    let mut records = Vec::new();
    let mut offset = 0;
    let mut previous_uid = 0;
    while offset < bytes.len() {
        let start = offset;
        let decoded = match decode_frame(bytes, start) {
            Ok(decoded) => decoded,
            Err(FrameError::Incomplete) => {
                return Ok(Scan {
                    records,
                    incomplete_tail: Some(start),
                });
            }
            Err(FrameError::Invalid(message)) => return Err(invalid(message)),
        };
        if decoded.uid <= previous_uid {
            return Err(invalid("non-monotonic uid"));
        }
        if decoded.name != blob_name(mailbox, decoded.uid) {
            return Err(invalid("unsafe blob reference"));
        }
        previous_uid = decoded.uid;
        offset = decoded.next;
        records.push(Record {
            uid: decoded.uid,
            name: decoded.name,
        });
    }
    Ok(Scan {
        records,
        incomplete_tail: None,
    })
}

struct DecodedFrame {
    uid: u64,
    name: String,
    next: usize,
}

enum FrameError {
    Incomplete,
    Invalid(&'static str),
}

/// Декодирует ровно один frame без выделения памяти по недоверенной длине.
fn decode_frame(bytes: &[u8], start: usize) -> Result<DecodedFrame, FrameError> {
    let mut decoder = Decoder::new(bytes, start);
    let magic = decoder.take::<4>()?;
    if magic != MAGIC {
        return Err(FrameError::Invalid("bad journal magic"));
    }
    let version = u16::from_be_bytes(decoder.take::<2>()?);
    if version != VERSION {
        return Err(FrameError::Invalid("unsupported journal version"));
    }
    let uid = u64::from_be_bytes(decoder.take::<8>()?);
    let transaction_id = u64::from_be_bytes(decoder.take::<8>()?);
    if transaction_id != uid {
        return Err(FrameError::Invalid("invalid transaction identity"));
    }
    let payload_length = u32::from_be_bytes(decoder.take::<4>()?) as usize;
    if payload_length > MAX_PAYLOAD {
        return Err(FrameError::Invalid("journal payload limit"));
    }
    let name = std::str::from_utf8(decoder.take_slice(payload_length)?)
        .map_err(|_| FrameError::Invalid("non-utf8 blob name"))?
        .to_owned();
    let checksum_position = decoder.position();
    let expected_checksum = u64::from_be_bytes(decoder.take::<8>()?);
    let commit = u32::from_be_bytes(decoder.take::<4>()?);
    if commit != COMMIT {
        return Err(FrameError::Invalid("missing journal commit marker"));
    }
    let protected = bytes
        .get(start..checksum_position)
        .ok_or(FrameError::Incomplete)?;
    if expected_checksum != checksum(protected) {
        return Err(FrameError::Invalid("journal checksum mismatch"));
    }
    Ok(DecodedFrame {
        uid,
        name,
        next: decoder.position(),
    })
}

/// Проверенный курсор исключает прямое индексирование недоверенных bytes.
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

    fn take<const SIZE: usize>(&mut self) -> Result<[u8; SIZE], FrameError> {
        let slice = self.take_slice(SIZE)?;
        slice.try_into().map_err(|_| FrameError::Incomplete)
    }

    fn take_slice(&mut self, length: usize) -> Result<&'a [u8], FrameError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(FrameError::Invalid("journal offset overflow"))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or(FrameError::Incomplete)?;
        self.offset = end;
        Ok(slice)
    }
}

/// FNV-1a обнаруживает случайную порчу и не считается криптографической MAC.
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
