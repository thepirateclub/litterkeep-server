//! Проверка BINARYMIME и bounded-разбиение сообщения на RFC 3030 BDAT chunks.
use crate::mime::{parameters::MediaType, tree::Entity};
use std::{
    io::{self, BufRead, Write},
    iter::Peekable,
};

/// Максимальный размер одного исходящего binary chunk для ограниченного I/O.
pub(crate) const MAX_CHUNK_SIZE: usize = 64 * 1024;

/// Один участок MIME-сообщения и признак завершения всей передачи.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BinaryChunk<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) last: bool,
}

/// Ленивый chunk stream: не копирует message и корректно выдаёт `BDAT 0 LAST`.
pub(crate) struct BinaryChunks<'a> {
    pending: Peekable<std::slice::Chunks<'a, u8>>,
    empty_message_pending: bool,
}

impl<'a> Iterator for BinaryChunks<'a> {
    type Item = BinaryChunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.pending.next() {
            Some(bytes) => Some(BinaryChunk {
                bytes,
                last: self.pending.peek().is_none(),
            }),
            None if self.empty_message_pending => {
                self.empty_message_pending = false;
                Some(BinaryChunk {
                    bytes: &[],
                    last: true,
                })
            }
            None => None,
        }
    }
}

/// Создаёт bounded итератор с нулевым LAST для пустого RFC 3030 сообщения.
pub(crate) fn binary_chunks(message: &[u8]) -> BinaryChunks<'_> {
    BinaryChunks {
        pending: message.chunks(MAX_CHUNK_SIZE).peekable(),
        empty_message_pending: message.is_empty(),
    }
}

/// Определяет BINARYMIME только по успешно построенному typed MIME-дереву.
pub(crate) fn has_binary_transfer(message: &[u8]) -> bool {
    crate::mime::tree::parse(message).is_ok_and(|root| root.contains_binary_transfer_encoding())
}

/// Проверяет структуру и канонические text parts, оставляя остальные тела opaque.
pub(crate) fn validate_binary(message: &[u8]) -> bool {
    validate_binary_profile(message, true)
}

/// Проверяет BINARYMIME сообщение, включая письма без CTE `binary` parts.
#[cfg(target_os = "linux")]
pub(crate) fn validate_bdat_binary(message: &[u8]) -> bool {
    validate_binary_profile(message, false)
}

/// Разбирает MIME и, при необходимости, требует хотя бы один binary transfer.
fn validate_binary_profile(message: &[u8], require_binary_transfer: bool) -> bool {
    let Ok(root) = crate::mime::tree::parse(message) else {
        return false;
    };
    match (
        root.contains_binary_transfer_encoding(),
        require_binary_transfer,
    ) {
        (true, _) | (false, false) => validate_entity(&root, message),
        (false, true) => false,
    }
}

/// Передаёт chunks строго последовательно и ждёт отдельный 250 после каждого.
pub(crate) fn send<R, F>(
    reader: &mut R,
    message: &[u8],
    mut write_chunk: F,
) -> io::Result<super::ReplyResult>
where
    R: BufRead,
    F: FnMut(&mut R, &[u8], &[u8]) -> io::Result<()>,
{
    for chunk in binary_chunks(message) {
        let marker = match chunk.last {
            true => " LAST",
            false => "",
        };
        let command = format!("BDAT {}{}\r\n", chunk.bytes.len(), marker);
        write_chunk(reader, command.as_bytes(), chunk.bytes)?;
        match super::expect(reader, super::ReplyExpectation::Bdat)? {
            super::ReplyResult::Accepted if chunk.last => {
                return Ok(super::ReplyResult::Accepted);
            }
            super::ReplyResult::Accepted => {}
            failure => return Ok(failure),
        }
    }
    Err(io::Error::other("BDAT chunk iterator ended without LAST"))
}

/// Адаптирует общий BDAT sender к plain TCP с отдельными reader/writer handles.
pub(crate) fn send_plain<R, W>(
    reader: &mut R,
    writer: &mut W,
    message: &[u8],
) -> io::Result<super::ReplyResult>
where
    R: BufRead,
    W: Write,
{
    send(reader, message, |_, command, payload| {
        writer.write_all(command)?;
        writer.write_all(payload)?;
        writer.flush()
    })
}

/// Преобразует BDAT reply в queue result и помечает закрывающийся socket.
pub(crate) fn delivery_outcome(
    reply: super::ReplyResult,
    close_after_response: &mut bool,
) -> super::DeliveryOutcome {
    match reply {
        super::ReplyResult::Accepted => super::DeliveryOutcome::Delivered,
        super::ReplyResult::Rejected(outcome) => outcome,
        super::ReplyResult::Closing => {
            *close_after_response = true;
            super::DeliveryOutcome::Deferred
        }
    }
}

/// Проверяет MIME entity без интерпретации байтов leaf с CTE `binary`.
fn validate_entity(entity: &Entity, message: &[u8]) -> bool {
    let headers_valid = entity
        .source_range(message, entity.header_range.clone())
        .is_some_and(|headers| crate::outbound::validate_data(headers).is_ok());
    if !headers_valid {
        return false;
    }
    let preamble_valid = validate_optional_text_range(&entity.preamble_range, entity, message);
    let epilogue_valid = validate_optional_text_range(&entity.epilogue_range, entity, message);
    let children_valid = entity
        .children
        .iter()
        .all(|child| validate_entity(child, message));
    match (entity.children.is_empty(), children_valid) {
        (false, valid) => valid && preamble_valid && epilogue_valid,
        (true, false) => false,
        (true, true) => validate_leaf(entity, message),
    }
}

/// Применяет строгие line rules к text/encoded leaves, но не к binary octets.
fn validate_leaf(entity: &Entity, message: &[u8]) -> bool {
    let Some(body) = entity.source_range(message, entity.body_range.clone()) else {
        return false;
    };
    match is_text(&entity.media_type) {
        true => canonical_text(body),
        false => true,
    }
}

/// Multipart delimiter-ы отделяют текстовые preamble/epilogue от opaque parts.
fn validate_optional_text_range(
    range: &Option<std::ops::Range<usize>>,
    entity: &Entity,
    message: &[u8],
) -> bool {
    match range {
        Some(range) => entity
            .source_range(message, range.clone())
            .is_some_and(has_only_canonical_line_breaks),
        None => true,
    }
}

/// Проверяет перенос строки в text part и требует canonical CRLF окончания.
fn canonical_text(body: &[u8]) -> bool {
    match body.is_empty() {
        true => true,
        false => body.ends_with(b"\r\n") && has_only_canonical_line_breaks(body),
    }
}

/// BINARYMIME не переносит DATA limit 1000 октетов на text/* bodies.
fn has_only_canonical_line_breaks(body: &[u8]) -> bool {
    let mut previous_was_cr = false;
    for octet in body.iter().copied() {
        match (previous_was_cr, octet) {
            (true, b'\n') => previous_was_cr = false,
            (true, _) => return false,
            (false, b'\r') => previous_was_cr = true,
            (false, b'\n') => return false,
            (false, _) => {}
        }
    }
    !previous_was_cr
}

/// MIME-категория уже разобрана, поэтому text-family выбирается через enum.
fn is_text(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::TextPlain | MediaType::TextHtml | MediaType::TextOther(_)
    )
}

#[cfg(test)]
#[path = "outbound_bdat_tests.rs"]
mod tests;
