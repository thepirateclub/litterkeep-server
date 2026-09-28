//! Проверка порядка кадров, версии и контрольной суммы SMTP DSN.

use super::cursor::{self, Cursor, FrameError};
use super::format::{self, MAX_FRAME};
use crate::smtp::DsnEnvelope;
use std::io;

/// Декодирует полные frames и отклоняет повреждение до финального tail.
pub(super) fn scan(bytes: &[u8]) -> io::Result<Vec<(u64, DsnEnvelope)>> {
    let mut records = Vec::new();
    let mut offset = 0;
    let mut previous = 0;
    while offset < bytes.len() {
        let decoded = match decode_frame(bytes, offset) {
            Ok(decoded) => decoded,
            Err(FrameError::Incomplete) => {
                return Err(cursor::invalid("incomplete SMTP DSN frame"));
            }
            Err(FrameError::Invalid(error)) => return Err(error),
        };
        if decoded.transaction_id <= previous {
            return Err(cursor::invalid("non-monotonic SMTP DSN transaction id"));
        }
        previous = decoded.transaction_id;
        offset = decoded.next;
        records.push((decoded.transaction_id, decoded.envelope));
    }
    Ok(records)
}

/// Находит начало последнего незавершённого frame после проверки prefix.
pub(super) fn incomplete_tail(bytes: &[u8]) -> io::Result<Option<usize>> {
    let mut offset = 0;
    let mut previous = 0;
    while offset < bytes.len() {
        match decode_frame(bytes, offset) {
            Ok(decoded) => {
                if decoded.transaction_id <= previous {
                    return Err(cursor::invalid("non-monotonic SMTP DSN transaction id"));
                }
                previous = decoded.transaction_id;
                offset = decoded.next;
            }
            Err(FrameError::Incomplete) => return Ok(Some(offset)),
            Err(FrameError::Invalid(error)) => return Err(error),
        }
    }
    Ok(None)
}

/// Проверяет magic, version, длину, checksum и commit marker одного frame.
fn decode_frame(bytes: &[u8], start: usize) -> Result<DecodedFrame, FrameError> {
    let mut cursor = Cursor::new(bytes, start);
    if cursor.take::<4>()? != format::MAGIC {
        return Err(FrameError::Invalid(cursor::invalid(
            "bad SMTP DSN journal magic",
        )));
    }
    if u16::from_be_bytes(cursor.take::<2>()?) != format::VERSION {
        return Err(FrameError::Invalid(cursor::invalid(
            "unsupported SMTP DSN journal version",
        )));
    }
    let transaction_id = u64::from_be_bytes(cursor.take::<8>()?);
    let length = u32::from_be_bytes(cursor.take::<4>()?) as usize;
    if length > MAX_FRAME.saturating_sub(30) {
        return Err(FrameError::Invalid(cursor::invalid(
            "SMTP DSN payload limit",
        )));
    }
    let payload_bytes = cursor.take_slice(length)?;
    let protected_end = cursor.position();
    let expected = u64::from_be_bytes(cursor.take::<8>()?);
    if u32::from_be_bytes(cursor.take::<4>()?) != format::COMMIT {
        return Err(FrameError::Invalid(cursor::invalid(
            "bad SMTP DSN commit marker",
        )));
    }
    let protected = bytes
        .get(start..protected_end)
        .ok_or(FrameError::Incomplete)?;
    if format::checksum(protected) != expected {
        return Err(FrameError::Invalid(cursor::invalid(
            "SMTP DSN checksum mismatch",
        )));
    }
    let envelope = super::payload::decode(payload_bytes)?;
    Ok(DecodedFrame {
        transaction_id,
        envelope,
        next: cursor.position(),
    })
}

struct DecodedFrame {
    transaction_id: u64,
    envelope: DsnEnvelope,
    next: usize,
}
