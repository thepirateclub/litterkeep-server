//! Ограниченный курсор для чтения кадров журнала DSN.

use std::io;

/// Ошибка различает неполный хвост и повреждённую запись.
pub(super) enum FrameError {
    Incomplete,
    Invalid(io::Error),
}

/// Курсор никогда не читает за пределами доступного буфера.
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    /// Создаёт курсор с абсолютной начальной позицией.
    pub(super) fn new(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }

    /// Считывает фиксированное количество байтов без прямой индексации.
    pub(super) fn take<const N: usize>(&mut self) -> Result<[u8; N], FrameError> {
        let slice = self.take_slice(N)?;
        slice.try_into().map_err(|_| FrameError::Incomplete)
    }

    /// Возвращает ограниченный срез и продвигает курсор.
    pub(super) fn take_slice(&mut self, length: usize) -> Result<&'a [u8], FrameError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| FrameError::Invalid(invalid("SMTP DSN cursor overflow")))?;
        let slice = self
            .bytes
            .get(self.position..end)
            .ok_or(FrameError::Incomplete)?;
        self.position = end;
        Ok(slice)
    }

    /// Возвращает текущую абсолютную позицию.
    pub(super) fn position(&self) -> usize {
        self.position
    }
}

/// Создаёт ошибку повреждённого durable формата.
pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
