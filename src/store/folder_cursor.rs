//! Проверенный курсор бинарного folder journal без прямой индексации bytes.

use std::io;

pub(super) struct Cursor<'a> {
    pub(super) bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn position(&self) -> usize {
        self.offset
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub(super) fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        self.take_slice(N)?.try_into().map_err(|_| short())
    }

    pub(super) fn take_slice(&mut self, length: usize) -> io::Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "folder offset overflow"))?;
        let value = self.bytes.get(self.offset..end).ok_or_else(short)?;
        self.offset = end;
        Ok(value)
    }
}

pub(super) fn short() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "short folder frame")
}
