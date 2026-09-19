//! Глобальная точка атомарной видимости multi-recipient delivery.
//!
//! Mailbox frames могут появиться до аварии, но становятся видимыми только
//! после durable commit transaction ID в этом последовательном журнале.
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

const MAGIC: [u8; 4] = *b"LKTC";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xc0de_ca11;
const FRAME_SIZE: usize = 26;

pub(super) struct Committed {
    pub(super) ids: HashSet<u64>,
    pub(super) maximum: u64,
}

/// Кодирует фиксированный commit frame без переменных и недоверенных длин.
pub(super) fn frame(transaction_id: u64) -> [u8; FRAME_SIZE] {
    let mut bytes = [0_u8; FRAME_SIZE];
    let mut offset = 0;
    write(&mut bytes, &mut offset, &MAGIC);
    write(&mut bytes, &mut offset, &VERSION.to_be_bytes());
    write(&mut bytes, &mut offset, &transaction_id.to_be_bytes());
    let checksum = checksum(bytes.get(..offset).expect("written prefix"));
    write(&mut bytes, &mut offset, &checksum.to_be_bytes());
    write(&mut bytes, &mut offset, &COMMIT.to_be_bytes());
    bytes
}

/// Читает монотонное множество committed transaction IDs.
pub(super) fn committed(path: &Path) -> io::Result<Committed> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let (ids, maximum, incomplete) = scan(&bytes)?;
    match incomplete {
        Some(_) => Err(invalid("incomplete transaction commit frame")),
        None => Ok(Committed { ids, maximum }),
    }
}

/// Обрезает только неполный последний commit frame.
pub(super) fn recover(path: &Path) -> io::Result<()> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let (_, _, incomplete) = scan(&bytes)?;
    let Some(valid_length) = incomplete else {
        return Ok(());
    };
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(valid_length as u64)?;
    file.sync_all()
}

fn scan(bytes: &[u8]) -> io::Result<(HashSet<u64>, u64, Option<usize>)> {
    let mut ids = HashSet::new();
    let mut maximum = 0_u64;
    let mut offset = 0;
    while offset < bytes.len() {
        let end = offset
            .checked_add(FRAME_SIZE)
            .ok_or_else(|| invalid("commit offset overflow"))?;
        let Some(frame) = bytes.get(offset..end) else {
            return Ok((ids, maximum, Some(offset)));
        };
        let transaction_id = decode(frame)?;
        if transaction_id <= maximum {
            return Err(invalid("non-monotonic transaction id"));
        }
        ids.insert(transaction_id);
        maximum = transaction_id;
        offset = end;
    }
    Ok((ids, maximum, None))
}

fn decode(frame: &[u8]) -> io::Result<u64> {
    let mut decoder = Decoder::new(frame);
    if decoder.take::<4>()? != MAGIC {
        return Err(invalid("bad transaction magic"));
    }
    if u16::from_be_bytes(decoder.take::<2>()?) != VERSION {
        return Err(invalid("unsupported transaction version"));
    }
    let transaction_id = u64::from_be_bytes(decoder.take::<8>()?);
    let protected_length = decoder.position();
    let expected_checksum = u64::from_be_bytes(decoder.take::<8>()?);
    if u32::from_be_bytes(decoder.take::<4>()?) != COMMIT {
        return Err(invalid("bad transaction commit marker"));
    }
    let protected = frame
        .get(..protected_length)
        .ok_or_else(|| invalid("bad transaction boundary"))?;
    if expected_checksum != checksum(protected) {
        return Err(invalid("transaction checksum mismatch"));
    }
    Ok(transaction_id)
}

fn write<const SIZE: usize>(target: &mut [u8], offset: &mut usize, value: &[u8; SIZE]) {
    let end = offset.checked_add(SIZE).expect("fixed frame capacity");
    target
        .get_mut(*offset..end)
        .expect("fixed frame boundary")
        .copy_from_slice(value);
    *offset = end;
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn position(&self) -> usize {
        self.offset
    }

    fn take<const SIZE: usize>(&mut self) -> io::Result<[u8; SIZE]> {
        let end = self
            .offset
            .checked_add(SIZE)
            .ok_or_else(|| invalid("transaction offset overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid("short transaction frame"))?
            .try_into()
            .map_err(|_| invalid("short transaction field"))?;
        self.offset = end;
        Ok(value)
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
