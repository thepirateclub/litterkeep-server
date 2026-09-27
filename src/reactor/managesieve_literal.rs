//! Отдельный RFC 5804 framing для bounded ManageSieve literal arguments.
//!
//! В отличие от IMAP, marker `\{n+\}\r\n` завершает командную строку, а
//! непосредственно после n байтов может следовать новый pipeline command.
//! Поэтому receiver не ждёт фиктивный завершающий CRLF и не смешивает правила
//! двух разных wire-протоколов.

use crate::managesieve::Command;

use super::managesieve_rename_literal as rename_literal;

const MAX_LITERAL_BYTES: usize = 65_536;

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
    PutName,
    PutNameTail(String),
    PutSource(String),
    Check,
    Noop,
    GetScript,
    DeleteScript,
    SetActive,
    Rename(rename_literal::Receiver),
}

#[path = "managesieve_literal_putscript.rs"]
mod putscript;

/// Единственный bounded владелец байтов literal между TCP reads.
pub(super) struct LiteralReceiver {
    operation: Operation,
    expected: usize,
    bytes: Vec<u8>,
}

/// Один неблокирующий шаг получения source, без чтения из TCP.
pub(super) enum Progress {
    Waiting,
    NeedContinuation,
    Invalid,
    InvalidUtf8,
    Complete(Command),
}

impl LiteralReceiver {
    pub(super) fn source(name: String, expected: usize) -> Self {
        Self::new(Operation::PutSource(name), expected)
    }

    fn new(operation: Operation, expected: usize) -> Self {
        Self {
            operation,
            expected,
            bytes: Vec::with_capacity(expected),
        }
    }

    /// Забирает ровно объявленное число байтов, оставляя pipeline в input.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        if let Operation::Rename(receiver) = &mut self.operation {
            return match receiver.consume(input) {
                rename_literal::Progress::Complete(command) => Progress::Complete(command),
                rename_literal::Progress::Waiting => Progress::Waiting,
                rename_literal::Progress::Continue => Progress::NeedContinuation,
                rename_literal::Progress::Invalid => Progress::Invalid,
                rename_literal::Progress::InvalidUtf8 => Progress::InvalidUtf8,
            };
        }
        if matches!(&self.operation, Operation::PutNameTail(_)) {
            return putscript::consume_tail(self, input);
        }
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
            Operation::PutName => putscript::begin_tail(self, source, input),
            Operation::PutNameTail(_) => Progress::Invalid,
            Operation::PutSource(name) => Progress::Complete(Command::PutScript { name, source }),
            Operation::Rename(_) => Progress::Invalid,
            Operation::Check => Progress::Complete(Command::CheckScript(source)),
            Operation::Noop => Progress::Complete(Command::Noop(Some(source))),
            Operation::GetScript => Progress::Complete(Command::GetScript(source)),
            Operation::DeleteScript => Progress::Complete(Command::DeleteScript(source)),
            Operation::SetActive => {
                let name = match source.is_empty() {
                    true => None,
                    false => Some(source),
                };
                Progress::Complete(Command::SetActive(name))
            }
        }
    }
}

/// Выбирает framing для команд с поддерживаемой literal-позицией.
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
        Verb::Noop => noop(tail),
        Verb::GetScript => string_argument(Operation::GetScript, tail),
        Verb::DeleteScript => string_argument(Operation::DeleteScript, tail),
        Verb::SetActive => string_argument(Operation::SetActive, tail),
        Verb::Rename => match rename_literal::start(tail) {
            rename_literal::Start::NotLiteral => Start::NotLiteral,
            rename_literal::Start::Invalid => Start::Invalid,
            rename_literal::Start::Begin { receiver, sync } => Start::Begin {
                receiver: LiteralReceiver {
                    operation: Operation::Rename(receiver),
                    expected: 0,
                    bytes: Vec::new(),
                },
                continuation: sync,
            },
        },
        Verb::Other => Start::NotLiteral,
    }
}

enum Verb {
    Put,
    Check,
    Noop,
    GetScript,
    DeleteScript,
    SetActive,
    Rename,
    Other,
}

impl Verb {
    fn parse(value: &str) -> Self {
        match () {
            _ if value.eq_ignore_ascii_case("PUTSCRIPT") => Self::Put,
            _ if value.eq_ignore_ascii_case("CHECKSCRIPT") => Self::Check,
            _ if value.eq_ignore_ascii_case("NOOP") => Self::Noop,
            _ if value.eq_ignore_ascii_case("GETSCRIPT") => Self::GetScript,
            _ if value.eq_ignore_ascii_case("DELETESCRIPT") => Self::DeleteScript,
            _ if value.eq_ignore_ascii_case("SETACTIVE") => Self::SetActive,
            _ if value.eq_ignore_ascii_case("RENAMESCRIPT") => Self::Rename,
            _ => Self::Other,
        }
    }
}

fn put(tail: &str) -> Start {
    match quoted_then_tail(tail) {
        Some((name, marker)) => literal(Operation::Put(name), marker),
        None => literal(Operation::PutName, tail.trim()),
    }
}

fn check(tail: &str) -> Start {
    literal(Operation::Check, tail.trim())
}

/// Распознаёт одиночный literal-тег NOOP; любые лишние аргументы отвергаются.
fn noop(tail: &str) -> Start {
    literal(Operation::Noop, tail.trim())
}

/// Обрабатывает команды с одним строковым финальным аргументом.
fn string_argument(operation: Operation, tail: &str) -> Start {
    literal(operation, tail.trim())
}

fn literal(operation: Operation, marker: &str) -> Start {
    let Some((expected, continuation)) = size(marker) else {
        return Start::NotLiteral;
    };
    match expected <= MAX_LITERAL_BYTES {
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
#[path = "managesieve_literal_tests.rs"]
mod tests;
