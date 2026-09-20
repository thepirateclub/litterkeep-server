//! Отдельный RFC 5804 framing для bounded ManageSieve script literal.
//!
//! В отличие от IMAP, marker `\{n+\}\r\n` завершает командную строку, а
//! непосредственно после n байтов может следовать новый pipeline command.
//! Поэтому receiver не ждёт фиктивный завершающий CRLF и не смешивает правила
//! двух разных wire-протоколов.

use crate::managesieve::Command;

const MAX_SCRIPT_BYTES: usize = 65_536;

/// Результат разбора command line до начала владения телом literal.
pub(super) enum Start {
    NotLiteral,
    Invalid,
    Begin {
        receiver: LiteralReceiver,
        continuation: bool,
    },
}

enum Operation {
    Put(String),
    Check,
}

/// Единственный bounded владелец байтов script между TCP reads.
pub(super) struct LiteralReceiver {
    operation: Operation,
    expected: usize,
    bytes: Vec<u8>,
}

/// Один неблокирующий шаг получения source, без чтения из TCP.
pub(super) enum Progress {
    Waiting,
    InvalidUtf8,
    Complete(Command),
}

impl LiteralReceiver {
    fn new(operation: Operation, expected: usize) -> Self {
        Self {
            operation,
            expected,
            bytes: Vec::with_capacity(expected),
        }
    }

    /// Забирает ровно объявленное число байтов, оставляя pipeline в input.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        let remaining = self.expected.saturating_sub(self.bytes.len());
        let consumed = remaining.min(input.len());
        self.bytes.extend(input.drain(..consumed));
        if self.bytes.len() < self.expected {
            return Progress::Waiting;
        }
        let source = match String::from_utf8(std::mem::take(&mut self.bytes)) {
            Ok(source) => source,
            Err(_) => return Progress::InvalidUtf8,
        };
        match std::mem::replace(&mut self.operation, Operation::Check) {
            Operation::Put(name) => Progress::Complete(Command::PutScript { name, source }),
            Operation::Check => Progress::Complete(Command::CheckScript(source)),
        }
    }
}

/// Разбирает лишь две команды, у которых последний аргумент является literal.
pub(super) fn start(line: &[u8]) -> Start {
    let Ok(text) = std::str::from_utf8(line) else {
        return Start::NotLiteral;
    };
    let Some(text) = text.strip_suffix("\r\n") else {
        return Start::Invalid;
    };
    let (verb, tail) = split_word(text);
    match Verb::parse(verb) {
        Verb::Put => put(tail),
        Verb::Check => check(tail),
        Verb::Other => Start::NotLiteral,
    }
}

enum Verb {
    Put,
    Check,
    Other,
}

impl Verb {
    fn parse(value: &str) -> Self {
        match () {
            _ if value.eq_ignore_ascii_case("PUTSCRIPT") => Self::Put,
            _ if value.eq_ignore_ascii_case("CHECKSCRIPT") => Self::Check,
            _ => Self::Other,
        }
    }
}

fn put(tail: &str) -> Start {
    let Some((name, marker)) = quoted_then_tail(tail) else {
        return Start::NotLiteral;
    };
    literal(Operation::Put(name), marker)
}

fn check(tail: &str) -> Start {
    literal(Operation::Check, tail.trim())
}

fn literal(operation: Operation, marker: &str) -> Start {
    let Some((expected, continuation)) = size(marker) else {
        return Start::NotLiteral;
    };
    match expected <= MAX_SCRIPT_BYTES {
        true => Start::Begin {
            receiver: LiteralReceiver::new(operation, expected),
            continuation,
        },
        false => Start::Invalid,
    }
}

/// Возвращает размер и признак синхронизирующего marker без плюса.
fn size(marker: &str) -> Option<(usize, bool)> {
    let marker = marker.trim();
    let body = marker.strip_prefix('{')?.strip_suffix('}')?;
    match body.strip_suffix('+') {
        Some(value) => value.parse().ok().map(|size| (size, false)),
        None => body.parse().ok().map(|size| (size, true)),
    }
}

fn split_word(text: &str) -> (&str, &str) {
    let trimmed = text.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(position) => match trimmed.get(..position) {
            Some(verb) => (
                verb,
                trimmed.get(position..).unwrap_or_default().trim_start(),
            ),
            None => ("", ""),
        },
        None => (trimmed, ""),
    }
}

fn quoted_then_tail(text: &str) -> Option<(String, &str)> {
    let mut characters = text.trim_start().chars();
    match characters.next() {
        Some('"') => {}
        _ => return None,
    }
    let mut name = String::new();
    loop {
        match characters.next()? {
            '"' => return Some((name, characters.as_str().trim())),
            '\\' => match characters.next()? {
                value @ ('"' | '\\') => name.push(value),
                _ => return None,
            },
            value => name.push(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Любая TCP-граница даёт тот же PUTSCRIPT и не съедает pipeline command.
    #[test]
    fn preserves_all_two_chunk_boundaries_and_pipeline() {
        let Start::Begin { .. } = start(b"PUTSCRIPT \"box\" {5+}\r\n") else {
            panic!("literal command was not recognized");
        };
        let wire = b"keep;NOOP\r\n";
        for split in 0..=wire.len() {
            let mut receiver = LiteralReceiver::new(Operation::Put("box".into()), 5);
            let mut input = wire.get(..split).expect("prefix").to_vec();
            let first_progress = receiver.consume(&mut input);
            input.extend_from_slice(wire.get(split..).expect("suffix"));
            let progress = match first_progress {
                Progress::Waiting => receiver.consume(&mut input),
                complete => complete,
            };
            let Progress::Complete(Command::PutScript { name, source }) = progress else {
                panic!("literal did not complete at split {split}");
            };
            assert_eq!(name, "box");
            assert_eq!(source, "keep;");
            assert_eq!(input, b"NOOP\r\n");
        }
    }

    #[test]
    fn recognizes_sync_literal_and_rejects_oversized_body() {
        assert!(matches!(
            start(b"CHECKSCRIPT {4}\r\n"),
            Start::Begin {
                continuation: true,
                ..
            }
        ));
        assert!(matches!(start(b"CHECKSCRIPT {65537+}\r\n"), Start::Invalid));
    }
}
