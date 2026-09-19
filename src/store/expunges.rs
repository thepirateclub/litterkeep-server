//! Долговечные tombstone-события окончательного удаления сообщений IMAP.

use super::durability;
use super::model::Record;
use super::{Store, WriteState};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const MAGIC: [u8; 4] = *b"LKEX";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xe8e8_c011;
const HEADER: usize = 10;
const TRAILER: usize = 12;

pub(super) fn load(root: &Path, entries: &mut HashMap<String, Vec<Record>>) -> io::Result<()> {
    for (mailbox, records) in entries.iter_mut() {
        let path = root.join("mailboxes").join(format!("{mailbox}.expunges"));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let (uids, valid_length) = decode_all(&bytes, records)?;
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
        let frame = encode(&removed)?;
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
        if let Some(records) = self.entries.get_mut(mailbox) {
            records.retain(|record| !removed_set.contains(&record.uid));
        }
        Ok(removed)
    }
}

fn encode(uids: &[u64]) -> io::Result<Vec<u8>> {
    let count = u32::try_from(uids.len()).map_err(|_| invalid("too many expunges"))?;
    let mut frame = Vec::with_capacity(HEADER + uids.len() * 8 + TRAILER);
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&VERSION.to_be_bytes());
    frame.extend_from_slice(&count.to_be_bytes());
    for uid in uids {
        frame.extend_from_slice(&uid.to_be_bytes());
    }
    frame.extend_from_slice(&checksum(&frame).to_be_bytes());
    frame.extend_from_slice(&COMMIT.to_be_bytes());
    Ok(frame)
}

fn decode_all(bytes: &[u8], records: &[Record]) -> io::Result<(Vec<u64>, usize)> {
    let mut uids = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let Some((frame, next)) = frame_at(bytes, offset)? else {
            return Ok((uids, offset));
        };
        for uid in frame {
            if !records.iter().any(|record| record.uid == uid) {
                return Err(invalid("expunge references unknown UID"));
            }
            if !seen.insert(uid) {
                return Err(invalid("duplicate expunge UID"));
            }
            uids.push(uid);
        }
        offset = next;
    }
    Ok((uids, offset))
}

fn frame_at(bytes: &[u8], offset: usize) -> io::Result<Option<(Vec<u64>, usize)>> {
    let header_end = offset.saturating_add(HEADER);
    let Some(header) = bytes.get(offset..header_end) else {
        return Ok(None);
    };
    let (magic, version, count) = decode_header(header)?;
    if (magic, version) != (MAGIC, VERSION) {
        return Err(invalid("unsupported expunge frame"));
    }
    let body = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or_else(|| invalid("expunge frame overflow"))?;
    let size = HEADER
        .checked_add(body)
        .and_then(|value| value.checked_add(TRAILER))
        .ok_or_else(|| invalid("expunge frame overflow"))?;
    let end = offset
        .checked_add(size)
        .ok_or_else(|| invalid("expunge frame overflow"))?;
    let Some(frame) = bytes.get(offset..end) else {
        return Ok(None);
    };
    decode_frame(frame).map(|uids| Some((uids, end)))
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

fn decode_frame(frame: &[u8]) -> io::Result<Vec<u64>> {
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
        .get(HEADER..)
        .ok_or_else(|| invalid("short expunge body"))?;
    let mut uids = Vec::with_capacity(uid_bytes.len() / 8);
    for chunk in uid_bytes.chunks_exact(8) {
        uids.push(u64::from_be_bytes(
            chunk.try_into().map_err(|_| invalid("short expunge UID"))?,
        ));
    }
    match uid_bytes.chunks_exact(8).remainder().is_empty() {
        true => Ok(uids),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail_flags::{FlagMutation, MessageFlags, SystemFlag};
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn expunge_hides_only_deleted_uid_and_survives_restart() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-expunge-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        store.append("bob", b"first").expect("first");
        store.append("bob", b"second").expect("second");
        store
            .update_flags(
                "bob",
                &[1],
                FlagMutation::Add,
                MessageFlags::from_flags([SystemFlag::Deleted]),
            )
            .expect("deleted flag");
        assert_eq!(store.expunge("bob", &[1]).expect("expunge"), vec![1]);
        assert_eq!(store.snapshot("bob").expect("snapshot").messages.len(), 1);
        drop(store);

        let journal = root.join("mailboxes").join("bob.expunges");
        let committed_length = fs::metadata(&journal).expect("metadata").len();
        let mut file = OpenOptions::new()
            .append(true)
            .open(&journal)
            .expect("journal");
        file.write_all(b"LKEX\0\x01\0\0\0\x01")
            .expect("partial tombstone");
        file.sync_all().expect("sync partial");
        drop(file);

        let reopened = Store::open(&root).expect("reopen");
        assert_eq!(
            fs::metadata(&journal).expect("trimmed metadata").len(),
            committed_length
        );
        let snapshot = reopened.snapshot("bob").expect("recovered snapshot");
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(
            snapshot.messages.first().map(|message| message.uid),
            Some(2)
        );
        assert_eq!(snapshot.uid_next, 3);
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
