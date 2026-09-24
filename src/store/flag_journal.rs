//! Кодек журнала изменяемых флагов с чтением старого формата v1.

use super::model::Record;
use crate::mail_flags::{KeywordFlag, MessageFlags, StoredFlags};
use std::collections::{BTreeSet, HashMap};
use std::io;

const MAGIC: [u8; 4] = *b"LKFL";
const VERSION: u16 = 2;
const COMMIT: u32 = 0xf1a6_c011;
const HEADER_SIZE: usize = 10;
const TRAILER_SIZE: usize = 12;

/// Кодирует полный набор состояния, чтобы mixed STORE имел один commit point.
pub(super) fn encode(updates: &[(u64, StoredFlags)]) -> io::Result<Vec<u8>> {
    let count = u32::try_from(updates.len()).map_err(|_| invalid("too many flag updates"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_be_bytes());
    bytes.extend_from_slice(&count.to_be_bytes());
    for (uid, flags) in updates {
        bytes.extend_from_slice(&uid.to_be_bytes());
        bytes.push(flags.system.bits());
        let count = u16::try_from(flags.keywords.len()).map_err(|_| invalid("keyword count"))?;
        bytes.extend_from_slice(&count.to_be_bytes());
        for keyword in &flags.keywords {
            let value = keyword.as_str().as_bytes();
            let length = u16::try_from(value.len()).map_err(|_| invalid("keyword length"))?;
            bytes.extend_from_slice(&length.to_be_bytes());
            bytes.extend_from_slice(value);
        }
    }
    let checksum = checksum(&bytes);
    bytes.extend_from_slice(&checksum.to_be_bytes());
    bytes.extend_from_slice(&COMMIT.to_be_bytes());
    Ok(bytes)
}

/// Восстанавливает последовательно зафиксированные записи и обрезаемый хвост.
pub(super) fn scan(
    bytes: &[u8],
    records: &[Record],
) -> io::Result<(HashMap<u64, StoredFlags>, usize)> {
    let mut flags = HashMap::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let Some(header) = bytes.get(offset..offset.saturating_add(HEADER_SIZE)) else {
            return Ok((flags, offset));
        };
        let (count, version) = decode_header(header)?;
        let Some(frame) = bytes.get(offset..) else {
            return Ok((flags, offset));
        };
        let (frame_size, updates) = match decode_frame(frame, count, version, records) {
            Ok(decoded) => decoded,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok((flags, offset));
            }
            Err(error) => return Err(error),
        };
        apply_updates(&mut flags, updates);
        offset = offset
            .checked_add(frame_size)
            .ok_or_else(|| invalid("flag offset overflow"))?;
    }
    Ok((flags, offset))
}

/// Декодирует заголовок и принимает только поддерживаемые версии.
fn decode_header(header: &[u8]) -> io::Result<(usize, u16)> {
    let mut cursor = Cursor::new(header);
    let magic = cursor.take::<4>()?;
    let version = u16::from_be_bytes(cursor.take::<2>()?);
    if magic != MAGIC || !matches!(version, 1 | VERSION) {
        return Err(invalid("unsupported flag frame"));
    }
    let count = usize::try_from(u32::from_be_bytes(cursor.take::<4>()?))
        .map_err(|_| invalid("flag count"))?;
    Ok((count, version))
}

/// Декодирует и checksum-проверяет один frame без частичной публикации.
fn decode_frame(
    frame: &[u8],
    count: usize,
    version: u16,
    records: &[Record],
) -> io::Result<(usize, Vec<(u64, StoredFlags)>)> {
    ensure_minimum_frame_length(frame, count, version)?;
    let mut cursor = Cursor::new(frame);
    cursor.take::<HEADER_SIZE>()?;
    let mut updates = Vec::with_capacity(count);
    for _ in 0..count {
        let uid = u64::from_be_bytes(cursor.take::<8>()?);
        let system = MessageFlags::from_bits(u8::from_be_bytes(cursor.take::<1>()?))?;
        let keywords = match version {
            1 => BTreeSet::new(),
            VERSION => decode_keywords(&mut cursor)?,
            _ => return Err(invalid("unsupported flag frame")),
        };
        validate_uid(records, uid)?;
        updates.push((uid, StoredFlags { system, keywords }));
    }
    let protected_end = cursor.offset;
    let expected = u64::from_be_bytes(cursor.take::<8>()?);
    let commit = u32::from_be_bytes(cursor.take::<4>()?);
    let protected = frame
        .get(..protected_end)
        .ok_or_else(|| invalid("short flag frame"))?;
    if (expected, commit) != (checksum(protected), COMMIT) {
        return Err(invalid("invalid flag frame trailer"));
    }
    Ok((cursor.offset, updates))
}

/// Отсекает невозможный счётчик до выделения памяти под обновления.
fn ensure_minimum_frame_length(frame: &[u8], count: usize, version: u16) -> io::Result<()> {
    let entry_size = match version {
        1 => 9,
        VERSION => 11,
        _ => return Err(invalid("unsupported flag frame")),
    };
    let minimum = count
        .checked_mul(entry_size)
        .and_then(|entries| entries.checked_add(HEADER_SIZE + TRAILER_SIZE))
        .ok_or_else(|| invalid("flag frame overflow"))?;
    match frame.len() >= minimum {
        true => Ok(()),
        false => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete flag frame",
        )),
    }
}

/// Читает ограниченные keyword atoms из недоверенного журнала.
fn decode_keywords(cursor: &mut Cursor<'_>) -> io::Result<BTreeSet<KeywordFlag>> {
    let count = usize::from(u16::from_be_bytes(cursor.take::<2>()?));
    let mut keywords = BTreeSet::new();
    for _ in 0..count {
        let length = usize::from(u16::from_be_bytes(cursor.take::<2>()?));
        let raw = std::str::from_utf8(cursor.take_slice(length)?)
            .map_err(|_| invalid("keyword utf-8"))?;
        let keyword = KeywordFlag::parse(raw).ok_or_else(|| invalid("keyword syntax"))?;
        if !keywords.insert(keyword) {
            return Err(invalid("duplicate keyword"));
        }
    }
    Ok(keywords)
}

/// Не позволяет повреждённой ссылке назначить флаги неизвестному UID.
fn validate_uid(records: &[Record], uid: u64) -> io::Result<()> {
    match records.iter().any(|record| record.uid == uid) {
        true => Ok(()),
        false => Err(invalid("flags reference unknown UID")),
    }
}

/// Публикует декодированный batch только после полной проверки checksum.
fn apply_updates(flags: &mut HashMap<u64, StoredFlags>, updates: Vec<(u64, StoredFlags)>) {
    flags.extend(updates);
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take<const SIZE: usize>(&mut self) -> io::Result<[u8; SIZE]> {
        self.take_slice(SIZE)?
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::UnexpectedEof, "short flag field"))
    }

    fn take_slice(&mut self, size: usize) -> io::Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| invalid("flag cursor overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short flag field"))?;
        self.offset = end;
        Ok(value)
    }
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |value, byte| {
        (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail_date::MailDate;

    fn record() -> Record {
        Record {
            uid: 1,
            transaction_id: 1,
            blob_transaction_id: 1,
            name: "message-1.eml".to_owned(),
            flags: MessageFlags::default(),
            keywords: BTreeSet::new(),
            internal_date: MailDate::from_unix_seconds(0),
        }
    }

    /// Старые frames читаются как системные flags с пустым keyword set.
    #[test]
    fn reads_legacy_v1_flag_frame() {
        let mut frame = Vec::new();
        frame.extend_from_slice(&MAGIC);
        frame.extend_from_slice(&1_u16.to_be_bytes());
        frame.extend_from_slice(&1_u32.to_be_bytes());
        frame.extend_from_slice(&1_u64.to_be_bytes());
        frame.push(1 << 3);
        let checksum = checksum(&frame);
        frame.extend_from_slice(&checksum.to_be_bytes());
        frame.extend_from_slice(&COMMIT.to_be_bytes());

        let (flags, valid_length) = scan(&frame, &[record()]).expect("legacy frame");
        let recovered = flags.get(&1).expect("recovered flags");
        assert!(
            recovered
                .system
                .contains(crate::mail_flags::SystemFlag::Seen)
        );
        assert!(recovered.keywords.is_empty());
        assert_eq!(valid_length, frame.len());
    }

    /// Недописанный новый frame не публикует ни одной записи batch-а.
    #[test]
    fn incomplete_v2_frame_is_a_recoverable_tail() {
        let keyword = KeywordFlag::parse("project-x").expect("keyword");
        let mut frame = encode(&[(
            1,
            StoredFlags {
                system: MessageFlags::default(),
                keywords: [keyword].into(),
            },
        )])
        .expect("frame");
        frame.truncate(frame.len().saturating_sub(1));

        let (flags, valid_length) = scan(&frame, &[record()]).expect("incomplete tail");
        assert!(flags.is_empty());
        assert_eq!(valid_length, 0);
    }
}
