//! Разбор аргументов PUTSCRIPT после literal-имени.

use super::{LiteralReceiver, MAX_LITERAL_BYTES, Operation, Progress};
use crate::managesieve::Command;

/// Переключает receiver после имени и сразу обрабатывает уже буферизованный хвост.
pub(super) fn begin_tail(
    receiver: &mut LiteralReceiver,
    name: String,
    input: &mut Vec<u8>,
) -> Progress {
    receiver.operation = Operation::PutNameTail(name);
    consume_tail(receiver, input)
}

/// Собирает ограниченный остаток строки, не поглощая следующую команду.
pub(super) fn consume_tail(receiver: &mut LiteralReceiver, input: &mut Vec<u8>) -> Progress {
    let name = match &receiver.operation {
        Operation::PutNameTail(name) => name.clone(),
        _ => return Progress::Invalid,
    };
    let bytes = &mut receiver.bytes;
    let Some(end) = input.windows(2).position(|pair| pair == b"\r\n") else {
        let remaining = MAX_LITERAL_BYTES.saturating_sub(bytes.len());
        let consumed = remaining.min(input.len());
        bytes.extend(input.drain(..consumed));
        return match bytes.len() < MAX_LITERAL_BYTES {
            true => Progress::Waiting,
            false => Progress::Invalid,
        };
    };
    let framed = end.saturating_add(2);
    if framed > MAX_LITERAL_BYTES.saturating_sub(bytes.len()) {
        return Progress::Invalid;
    }
    bytes.extend(input.drain(..framed));
    receiver.operation = Operation::Check;
    let tail = match String::from_utf8(std::mem::take(bytes)) {
        Ok(value) => value,
        Err(_) => return Progress::InvalidUtf8,
    };
    finish_tail(receiver, name, &tail, input)
}

fn finish_tail(
    receiver: &mut LiteralReceiver,
    name: String,
    frame: &str,
    input: &mut Vec<u8>,
) -> Progress {
    let Some(tail) = frame.strip_suffix("\r\n") else {
        return Progress::Invalid;
    };
    match quoted(tail.trim_start()) {
        Some((source, "")) => Progress::Complete(Command::PutScript { name, source }),
        Some(_) => Progress::Invalid,
        None => start_source_literal(receiver, name, tail.trim_start(), input),
    }
}

fn start_source_literal(
    receiver: &mut LiteralReceiver,
    name: String,
    marker: &str,
    input: &mut Vec<u8>,
) -> Progress {
    let Some((expected, sync)) = size(marker) else {
        return Progress::Invalid;
    };
    if expected > MAX_LITERAL_BYTES {
        return Progress::Invalid;
    }
    *receiver = LiteralReceiver::source(name, expected);
    match sync {
        true => Progress::NeedContinuation,
        false => receiver.consume(input),
    }
}

fn size(marker: &str) -> Option<(usize, bool)> {
    let body = marker.trim().strip_prefix('{')?.strip_suffix('}')?;
    match body.strip_suffix('+') {
        Some(value) => value.parse().ok().map(|size| (size, false)),
        None => body.parse().ok().map(|size| (size, true)),
    }
}

fn quoted(text: &str) -> Option<(String, &str)> {
    let mut chars = text.trim_start().chars();
    match chars.next() {
        Some('"') => {}
        _ => return None,
    }
    let mut value = String::new();
    loop {
        match chars.next()? {
            '"' => return Some((value, chars.as_str().trim())),
            '\\' => match chars.next()? {
                escaped @ ('"' | '\\') => value.push(escaped),
                _ => return None,
            },
            character => value.push(character),
        }
    }
}
