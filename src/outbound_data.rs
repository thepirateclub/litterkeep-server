//! Проверка и сериализация SMTP DATA без изменения message octets.

use std::io::{self, Write};

/// Причина, по которой RFC 5321 DATA нельзя передать без изменения octets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DataValidationError {
    LineTooLong,
    BareCarriageReturn,
    BareLineFeed,
    NonAsciiHeader,
}

/// Проверяет CRLF framing, line limit и ASCII-профиль заголовков до envelope.
pub(crate) fn validate_data(message: &[u8]) -> Result<(), DataValidationError> {
    validate_line_framing(message)?;
    match header_bytes(message) {
        Some(headers) if headers.iter().any(|byte| !byte.is_ascii()) => {
            Err(DataValidationError::NonAsciiHeader)
        }
        Some(_) => Ok(()),
        None if message.iter().any(|byte| !byte.is_ascii()) => {
            Err(DataValidationError::NonAsciiHeader)
        }
        None => Ok(()),
    }
}

/// Выявляет high-bit octets только в content body, а не в header fields.
pub(crate) fn has_8bit_body(message: &[u8]) -> bool {
    match body_bytes(message) {
        Some(body) => body.iter().any(|byte| !byte.is_ascii()),
        None => false,
    }
}

/// Считает message octets после добавления обязательного завершающего CRLF.
///
/// RFC 1870 не включает SMTP terminator и удвоенные transparency dots в SIZE.
pub(crate) fn transmitted_message_size(message: &[u8]) -> usize {
    match message.ends_with(b"\r\n") {
        true => message.len(),
        false => message.len().saturating_add(2),
    }
}

/// Потоково передаёт DATA с dot-stuffing и одним завершающим CRLF.
///
/// Вызывающий слой заранее проверяет CRLF framing и RFC-ограничение длины
/// строки. Поэтому разделение по LF удерживает каждый временный фрагмент в
/// пределах одной короткой SMTP-строки вместо копирования всего MIME payload.
/// Каждый фрагмент передаётся через `write_all`, сохраняя timeout системных
/// записей и корректность при частичной записи в underlying socket.
pub(crate) fn write_data(writer: &mut impl Write, message: &[u8]) -> io::Result<()> {
    for line in message.split_inclusive(|byte| *byte == b'\n') {
        if let Some(b'.') = line.first() {
            writer.write_all(b".")?;
        }
        writer.write_all(line)?;
    }
    if !message.ends_with(b"\r\n") {
        writer.write_all(b"\r\n")?;
    }
    writer.write_all(b".\r\n")
}

fn validate_line_framing(message: &[u8]) -> Result<(), DataValidationError> {
    let mut line_length = 0_usize;
    let mut pending_cr = false;
    for byte in message.iter().copied() {
        match (pending_cr, byte) {
            (true, b'\n') => {
                line_length = line_length.saturating_add(1);
                if line_length > 1000 {
                    return Err(DataValidationError::LineTooLong);
                }
                line_length = 0;
                pending_cr = false;
            }
            (true, _) => return Err(DataValidationError::BareCarriageReturn),
            (false, b'\r') => {
                line_length = line_length.saturating_add(1);
                pending_cr = true;
            }
            (false, b'\n') => return Err(DataValidationError::BareLineFeed),
            (false, _) => line_length = line_length.saturating_add(1),
        }
        if line_length > 1000 {
            return Err(DataValidationError::LineTooLong);
        }
    }
    if pending_cr {
        return Err(DataValidationError::BareCarriageReturn);
    }
    match line_length.saturating_add(2) > 1000 {
        true => Err(DataValidationError::LineTooLong),
        false => Ok(()),
    }
}

fn header_bytes(message: &[u8]) -> Option<&[u8]> {
    match message.starts_with(b"\r\n") {
        true => message.get(..0),
        false => message
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .and_then(|separator| message.get(..separator)),
    }
}

fn body_bytes(message: &[u8]) -> Option<&[u8]> {
    match message.starts_with(b"\r\n") {
        true => message.get(2..),
        false => message
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .and_then(|separator| message.get(separator.saturating_add(4)..)),
    }
}
