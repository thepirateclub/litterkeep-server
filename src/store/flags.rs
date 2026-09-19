//! Атомарный append-only журнал изменяемых флагов сообщений.

use super::model::Record;
use super::{Store, durability};
use crate::mail_flags::{FlagMutation, MessageFlags};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};

const MAGIC: [u8; 4] = *b"LKFL";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xf1a6_c011;
const HEADER_SIZE: usize = 10;
const ENTRY_SIZE: usize = 9;
const TRAILER_SIZE: usize = 12;

pub(super) fn load(
    root: &std::path::Path,
    entries: &HashMap<String, Vec<Record>>,
) -> io::Result<HashMap<String, HashMap<u64, MessageFlags>>> {
    let mut result = HashMap::new();
    for (mailbox, records) in entries {
        let path = root.join("mailboxes").join(format!("{mailbox}.flags"));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (flags, valid_length) = scan(&bytes, records)?;
        if valid_length < bytes.len() {
            let file = OpenOptions::new().write(true).open(path)?;
            file.set_len(valid_length as u64)?;
            file.sync_all()?;
        }
        result.insert(mailbox.clone(), flags);
    }
    Ok(result)
}

impl Store {
    /// Фиксирует изменение всех UID одним checksum-защищённым frame.
    pub fn update_flags(
        &mut self,
        mailbox: &str,
        uids: &[u64],
        operation: FlagMutation,
        operand: MessageFlags,
    ) -> io::Result<()> {
        self.ensure_writable()?;
        super::model::validate_mailbox(mailbox)?;
        let records = self.entries.get(mailbox).map(Vec::as_slice).unwrap_or(&[]);
        let mut unique_uids = uids.to_vec();
        unique_uids.sort_unstable();
        unique_uids.dedup();
        if unique_uids.iter().any(|uid| {
            records
                .binary_search_by_key(uid, |record| record.uid)
                .is_err()
        }) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "unknown message UID",
            ));
        }
        let existing = self.flags.get(mailbox);
        let updates = unique_uids
            .iter()
            .map(|uid| {
                (
                    *uid,
                    existing
                        .and_then(|current| current.get(uid))
                        .copied()
                        .unwrap_or_default()
                        .mutate(operation, operand),
                )
            })
            .collect::<Vec<_>>();
        if updates.is_empty() {
            return Ok(());
        }
        let frame = encode(&updates)?;
        let path = self.root.join("mailboxes").join(format!("{mailbox}.flags"));
        let durable = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(&frame).and_then(|()| file.sync_all()))
            .and_then(|()| durability::sync_directory(&self.root.join("mailboxes")));
        if let Err(error) = durable {
            self.write_state = super::WriteState::Poisoned;
            return Err(error);
        }
        self.flags
            .entry(mailbox.to_owned())
            .or_default()
            .extend(updates);
        Ok(())
    }
}

fn encode(updates: &[(u64, MessageFlags)]) -> io::Result<Vec<u8>> {
    let count = u32::try_from(updates.len()).map_err(|_| invalid("too many flag updates"))?;
    let mut bytes = Vec::with_capacity(HEADER_SIZE + updates.len() * ENTRY_SIZE + TRAILER_SIZE);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_be_bytes());
    bytes.extend_from_slice(&count.to_be_bytes());
    for (uid, flags) in updates {
        bytes.extend_from_slice(&uid.to_be_bytes());
        bytes.push(flags.bits());
    }
    let checksum = checksum(&bytes);
    bytes.extend_from_slice(&checksum.to_be_bytes());
    bytes.extend_from_slice(&COMMIT.to_be_bytes());
    Ok(bytes)
}

fn scan(bytes: &[u8], records: &[Record]) -> io::Result<(HashMap<u64, MessageFlags>, usize)> {
    let mut flags = HashMap::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let Some(header) = bytes.get(offset..offset.saturating_add(HEADER_SIZE)) else {
            return Ok((flags, offset));
        };
        let count = decode_header(header)?;
        let frame_size = HEADER_SIZE
            .checked_add(
                count
                    .checked_mul(ENTRY_SIZE)
                    .ok_or_else(|| invalid("flag frame overflow"))?,
            )
            .and_then(|size| size.checked_add(TRAILER_SIZE))
            .ok_or_else(|| invalid("flag frame overflow"))?;
        let Some(frame) = bytes.get(offset..offset.saturating_add(frame_size)) else {
            return Ok((flags, offset));
        };
        apply_frame(frame, count, records, &mut flags)?;
        offset = offset
            .checked_add(frame_size)
            .ok_or_else(|| invalid("flag offset overflow"))?;
    }
    Ok((flags, offset))
}

fn decode_header(header: &[u8]) -> io::Result<usize> {
    let mut cursor = Cursor::new(header);
    let identity = (cursor.take::<4>()?, u16::from_be_bytes(cursor.take::<2>()?));
    if identity != (MAGIC, VERSION) {
        return Err(invalid("unsupported flag frame"));
    }
    usize::try_from(u32::from_be_bytes(cursor.take::<4>()?)).map_err(|_| invalid("flag count"))
}

fn apply_frame(
    frame: &[u8],
    count: usize,
    records: &[Record],
    flags: &mut HashMap<u64, MessageFlags>,
) -> io::Result<()> {
    let protected = frame
        .get(..frame.len().saturating_sub(TRAILER_SIZE))
        .ok_or_else(|| invalid("short flag frame"))?;
    let mut cursor = Cursor::new(frame);
    cursor.take::<HEADER_SIZE>()?;
    let mut updates = Vec::with_capacity(count);
    for _ in 0..count {
        let uid = u64::from_be_bytes(cursor.take::<8>()?);
        let value = MessageFlags::from_bits(u8::from_be_bytes(cursor.take::<1>()?))?;
        if !records.iter().any(|record| record.uid == uid) {
            return Err(invalid("flags reference unknown UID"));
        }
        updates.push((uid, value));
    }
    let expected = u64::from_be_bytes(cursor.take::<8>()?);
    let trailer = (expected, u32::from_be_bytes(cursor.take::<4>()?));
    if trailer != (checksum(protected), COMMIT) {
        return Err(invalid("invalid flag frame trailer"));
    }
    flags.extend(updates);
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or_else(|| invalid("flag cursor overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid("short flag field"))?
            .try_into()
            .map_err(|_| invalid("short flag field"))?;
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
