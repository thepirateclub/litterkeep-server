//! Сериализация ответов POP3 с проверкой границ строк RFC 1939 и RFC 2449.

use super::{Reply, ResponseCode};

/// Максимум status/capability строки POP3 вместе с CRLF по RFC 2449.
pub(crate) const MAX_REPLY_LINE_SIZE: usize = 512;
const LONG_REPLY: &[u8] = b"-ERR response line too long\r\n";

/// Сериализует только готовые protocol replies; action variants остаются у reactor.
pub(crate) fn encode_reply(reply: Reply) -> Option<Vec<u8>> {
    match reply {
        Reply::Ok(text) => Some(single_line(b"+OK ", &text)),
        Reply::Error(text) => Some(single_line(b"-ERR ", text)),
        Reply::ErrorCode(code, text) => Some(extended_error(code, text)),
        Reply::SaslContinue => Some(b"+ \r\n".to_vec()),
        Reply::SaslChallenge(value) => Some(sasl_challenge(&value)),
        Reply::Multiline(bytes) => Some(bounded_multiline(bytes)),
        Reply::Message(bytes) => Some(message_multiline(bytes)),
        Reply::Authenticate { .. }
        | Reply::ScramVerifierLookup { .. }
        | Reply::ScramSha256VerifierLookup { .. }
        | Reply::ScramAccepted { .. }
        | Reply::CommitDeletes(_)
        | Reply::StartTls => None,
    }
}

/// Кодирует RFC 5034 server challenge с обязательным `+ ` prefix.
fn sasl_challenge(value: &str) -> Vec<u8> {
    let mut reply = Vec::with_capacity(value.len().saturating_add(5));
    reply.extend_from_slice(b"+ ");
    reply.extend_from_slice(value.as_bytes());
    reply.extend_from_slice(b"\r\n");
    reply
}

/// Формирует расширенный -ERR через типизированный код причины.
fn extended_error(code: ResponseCode, text: &str) -> Vec<u8> {
    let prefix = match code {
        ResponseCode::InUse => b"-ERR [IN-USE] ".as_slice(),
        ResponseCode::Utf8 => b"-ERR [UTF8] ".as_slice(),
    };
    single_line(prefix, text)
}

/// Ограничивает однострочный статус после формирования финальных UTF-8 bytes.
fn single_line(prefix: &[u8], text: &str) -> Vec<u8> {
    let mut line = Vec::with_capacity(prefix.len().saturating_add(text.len()).saturating_add(2));
    line.extend_from_slice(prefix);
    line.extend_from_slice(text.as_bytes());
    line.extend_from_slice(b"\r\n");
    match valid_status_line(&line) {
        true => line,
        false => LONG_REPLY.to_vec(),
    }
}

/// Проверяет status и каждую protocol data line в LIST/UIDL/CAPA frames.
fn bounded_multiline(bytes: Vec<u8>) -> Vec<u8> {
    match (has_status_header(&bytes), has_terminated_lines(&bytes)) {
        (true, true)
            if bytes
                .split_inclusive(|octet| *octet == b'\n')
                .all(|line| line.len() <= MAX_REPLY_LINE_SIZE) =>
        {
            bytes
        }
        _ => LONG_REPLY.to_vec(),
    }
}

/// Ограничивает только начальную status line, сохраняя RFC message line длину.
fn message_multiline(bytes: Vec<u8>) -> Vec<u8> {
    match (has_status_header(&bytes), has_terminated_lines(&bytes)) {
        (true, true) => match first_line(&bytes) {
            Some(line) if line.len() <= MAX_REPLY_LINE_SIZE => bytes,
            _ => LONG_REPLY.to_vec(),
        },
        _ => LONG_REPLY.to_vec(),
    }
}

/// Проверяет `+OK ` status, его printable text и включённые CRLF октеты.
fn has_status_header(bytes: &[u8]) -> bool {
    let Some(line) = first_line(bytes) else {
        return false;
    };
    line.starts_with(b"+OK ") && valid_status_line(line)
}

/// Валидирует все CRLF границы и обязательный dot terminator multiline frame.
fn has_terminated_lines(bytes: &[u8]) -> bool {
    bytes.ends_with(b".\r\n")
        && bytes
            .split_inclusive(|octet| *octet == b'\n')
            .all(|line| line.ends_with(b"\r\n"))
}

/// Извлекает первую CRLF строку без копирования или числового индекса.
fn first_line(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .split_inclusive(|octet| *octet == b'\n')
        .next()
        .filter(|line| line.ends_with(b"\r\n"))
}

/// Status text состоит из printable ASCII/space и ровно завершается CRLF.
fn valid_status_line(line: &[u8]) -> bool {
    line.ends_with(b"\r\n")
        && line.strip_suffix(b"\r\n").is_some_and(|text| {
            text.iter()
                .all(|octet| *octet == b' ' || (33..=126).contains(octet))
        })
        && line.len() <= MAX_REPLY_LINE_SIZE
}

#[cfg(test)]
#[path = "reply_wire_tests.rs"]
mod tests;
