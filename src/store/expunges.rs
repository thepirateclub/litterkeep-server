//! Долговечные tombstone-события окончательного удаления сообщений IMAP.
use super::durability;
use super::model::Record;
use super::{Store, WriteState, commit_log};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
const MAGIC: [u8; 4] = *b"LKEX";
const VERSION: u16 = 2;
const COMMIT: u32 = 0xe8e8_c011;
const LEGACY_HEADER: usize = 10;
const TRANSACTION_HEADER: usize = 18;
const TRAILER: usize = 12;
struct Frame {
    transaction_id: Option<u64>,
    uids: Vec<u64>,
}
impl Frame {
    fn applies(&self, committed: &commit_log::Committed) -> bool {
        match self.transaction_id {
            Some(transaction_id) => committed.ids.contains(&transaction_id),
            None => true,
        }
    }
}
pub(super) fn load(
    root: &Path,
    entries: &mut HashMap<String, Vec<Record>>,
    committed: &commit_log::Committed,
) -> io::Result<()> {
    for (mailbox, records) in entries.iter_mut() {
        let path = root.join("mailboxes").join(format!("{mailbox}.expunges"));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (uids, valid_length) = decode_all(&bytes, records, committed)?;
        if valid_length < bytes.len() {
            let file = OpenOptions::new().write(true).open(&path)?;
            file.set_len(valid_length as u64)?;
            file.sync_all()?;
        }
        let removed: HashSet<u64> = uids.into_iter().collect();
        records.retain(|record| !removed.contains(&record.uid));
    }
    Ok(())
}
impl Store {
    /// Фиксирует окончательное удаление известных UID одним fsync frame.
    pub fn expunge(&mut self, mailbox: &str, uids: &[u64]) -> io::Result<Vec<u64>> {
        self.ensure_writable()?;
        super::model::validate_mailbox(mailbox)?;
        let records = self.entries.get(mailbox).map(Vec::as_slice).unwrap_or(&[]);
        let mut removed = uids.to_vec();
        removed.sort_unstable();
        removed.dedup();
        if removed
            .iter()
            .any(|uid| !records.iter().any(|record| record.uid == *uid))
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "unknown expunge UID",
            ));
        }
        if removed.is_empty() {
            return Ok(removed);
        }
        let removed_set: HashSet<u64> = removed.iter().copied().collect();
        let frame = encode_legacy(&removed)?;
        let path = self
            .root
            .join("mailboxes")
            .join(format!("{mailbox}.expunges"));
        let durable = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(&frame).and_then(|()| file.sync_all()))
            .and_then(|()| durability::sync_directory(&self.root.join("mailboxes")));
        if let Err(error) = durable {
            self.write_state = WriteState::Poisoned;
            return Err(error);
        }
        self.discard_recent(mailbox, &removed)
            .map_err(|error| self.poison_after_recent_error(error))?;
        if let Some(records) = self.entries.get_mut(mailbox) {
            records.retain(|record| !removed_set.contains(&record.uid));
        }
        Ok(removed)
    }
}
pub(super) fn write_transaction(
    root: &Path,
    mailbox: &str,
    transaction_id: u64,
    uids: &[u64],
) -> io::Result<()> {
    let frame = encode_transaction(transaction_id, uids)?;
    let path = root.join("mailboxes").join(format!("{mailbox}.expunges"));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(&frame).and_then(|()| file.sync_all()))
}

fn encode_legacy(uids: &[u64]) -> io::Result<Vec<u8>> {
    let count = u32::try_from(uids.len()).map_err(|_| invalid("too many expunges"))?;
    let mut frame = Vec::with_capacity(LEGACY_HEADER + uids.len() * 8 + TRAILER);
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&1_u16.to_be_bytes());
    frame.extend_from_slice(&count.to_be_bytes());
    append_uids(&mut frame, uids);
    finish_frame(frame)
}

fn encode_transaction(transaction_id: u64, uids: &[u64]) -> io::Result<Vec<u8>> {
    let count = u32::try_from(uids.len()).map_err(|_| invalid("too many expunges"))?;
    let mut frame = Vec::with_capacity(TRANSACTION_HEADER + uids.len() * 8 + TRAILER);
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&VERSION.to_be_bytes());
    frame.extend_from_slice(&transaction_id.to_be_bytes());
    frame.extend_from_slice(&count.to_be_bytes());
    append_uids(&mut frame, uids);
    finish_frame(frame)
}

fn append_uids(frame: &mut Vec<u8>, uids: &[u64]) {
    for uid in uids {
        frame.extend_from_slice(&uid.to_be_bytes());
    }
}

fn finish_frame(mut frame: Vec<u8>) -> io::Result<Vec<u8>> {
    frame.extend_from_slice(&checksum(&frame).to_be_bytes());
    frame.extend_from_slice(&COMMIT.to_be_bytes());
    Ok(frame)
}

fn decode_all(
    bytes: &[u8],
    records: &[Record],
    committed: &commit_log::Committed,
) -> io::Result<(Vec<u64>, usize)> {
    let mut uids = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let Some((frame, next)) = frame_at(bytes, offset)? else {
            return Ok((uids, offset));
        };
        if frame.applies(committed) {
            for uid in frame.uids {
                if !records.iter().any(|record| record.uid == uid) {
                    return Err(invalid("expunge references unknown UID"));
                }
                if !seen.insert(uid) {
                    return Err(invalid("duplicate expunge UID"));
                }
                uids.push(uid);
            }
        }
        offset = next;
    }
    Ok((uids, offset))
}

fn frame_at(bytes: &[u8], offset: usize) -> io::Result<Option<(Frame, usize)>> {
    let header_end = offset.saturating_add(LEGACY_HEADER);
    let Some(header) = bytes.get(offset..header_end) else {
        return Ok(None);
    };
    let (magic, version, legacy_count) = decode_header(header)?;
    if magic != MAGIC {
        return Err(invalid("unsupported expunge frame"));
    }
    let (transaction_id, count, header_size) = match version {
        1 => (None, legacy_count, LEGACY_HEADER),
        VERSION => {
            let Some((transaction_id, count)) = decode_transaction_header(bytes, offset)? else {
                return Ok(None);
            };
            (Some(transaction_id), count, TRANSACTION_HEADER)
        }
        _ => return Err(invalid("unsupported expunge frame")),
    };
    let body = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or_else(|| invalid("expunge frame overflow"))?;
    let size = header_size
        .checked_add(body)
        .and_then(|value| value.checked_add(TRAILER))
        .ok_or_else(|| invalid("expunge frame overflow"))?;
    let end = offset
        .checked_add(size)
        .ok_or_else(|| invalid("expunge frame overflow"))?;
    let Some(frame) = bytes.get(offset..end) else {
        return Ok(None);
    };
    decode_frame(frame, header_size, transaction_id).map(|frame| Some((frame, end)))
}

fn decode_transaction_header(bytes: &[u8], offset: usize) -> io::Result<Option<(u64, u32)>> {
    let end = offset.saturating_add(TRANSACTION_HEADER);
    let Some(header) = bytes.get(offset..end) else {
        return Ok(None);
    };
    let transaction_id = header
        .get(6..14)
        .ok_or_else(|| invalid("short transaction tombstone"))?
        .try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| invalid("short transaction tombstone"))?;
    let count = header
        .get(14..18)
        .ok_or_else(|| invalid("short transaction tombstone"))?
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| invalid("short transaction tombstone"))?;
    Ok(Some((transaction_id, count)))
}

fn decode_header(header: &[u8]) -> io::Result<([u8; 4], u16, u32)> {
    let magic = header
        .get(..4)
        .ok_or_else(|| invalid("short expunge header"))?
        .try_into()
        .map_err(|_| invalid("short expunge magic"))?;
    let version = header
        .get(4..6)
        .ok_or_else(|| invalid("short expunge header"))?
        .try_into()
        .map(u16::from_be_bytes)
        .map_err(|_| invalid("short expunge version"))?;
    let count = header
        .get(6..10)
        .ok_or_else(|| invalid("short expunge header"))?
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| invalid("short expunge count"))?;
    Ok((magic, version, count))
}

fn decode_frame(
    frame: &[u8],
    header_size: usize,
    transaction_id: Option<u64>,
) -> io::Result<Frame> {
    let protected_end = frame.len().saturating_sub(TRAILER);
    let protected = frame
        .get(..protected_end)
        .ok_or_else(|| invalid("short expunge"))?;
    let trailer = frame
        .get(protected_end..)
        .ok_or_else(|| invalid("short trailer"))?;
    let expected = trailer
        .get(..8)
        .ok_or_else(|| invalid("short trailer"))?
        .try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| invalid("short checksum"))?;
    let commit = trailer
        .get(8..12)
        .ok_or_else(|| invalid("short trailer"))?
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| invalid("short commit"))?;
    if (expected, commit) != (checksum(protected), COMMIT) {
        return Err(invalid("invalid expunge trailer"));
    }
    let uid_bytes = protected
        .get(header_size..)
        .ok_or_else(|| invalid("short expunge body"))?;
    let mut uids = Vec::with_capacity(uid_bytes.len() / 8);
    for chunk in uid_bytes.chunks_exact(8) {
        uids.push(u64::from_be_bytes(
            chunk.try_into().map_err(|_| invalid("short expunge UID"))?,
        ));
    }
    match uid_bytes.chunks_exact(8).remainder().is_empty() {
        true => Ok(Frame {
            transaction_id,
            uids,
        }),
        false => Err(invalid("unaligned expunge UID")),
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
