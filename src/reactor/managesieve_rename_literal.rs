//! Ограниченное framing literal-аргументов RENAMESCRIPT по RFC 5804.

use crate::managesieve::Command;

const LIMIT: usize = 65_536;

/// Владение обоими именами и состоянием их независимых continuation.
pub(super) struct Receiver {
    old: String,
    expected: usize,
    bytes: Vec<u8>,
    phase: Phase,
}

enum Phase {
    Old,
    Tail,
    New,
}

pub(super) enum Start {
    NotLiteral,
    Invalid,
    Begin { receiver: Receiver, sync: bool },
}

pub(super) enum Progress {
    Waiting,
    Continue,
    Invalid,
    InvalidUtf8,
    Complete(Command),
}

/// Определяет первую literal-позицию, не перехватывая полностью quoted-команду.
pub(super) fn start(tail: &str) -> Start {
    match quoted(tail) {
        Some((old, rest)) => match quoted(rest) {
            Some((_, "")) => Start::NotLiteral,
            Some(_) => Start::Invalid,
            None => begin(old, rest, false),
        },
        None => begin(String::new(), tail.trim(), true),
    }
}

/// Создаёт bounded receiver с известным размером первого literal.
fn begin(old: String, marker: &str, old_is_literal: bool) -> Start {
    let phase = match old_is_literal {
        true => Phase::Old,
        false => Phase::New,
    };
    let Some((expected, sync)) = size(marker) else {
        return Start::NotLiteral;
    };
    match expected <= LIMIT {
        true => Start::Begin {
            receiver: Receiver {
                old,
                expected,
                bytes: Vec::with_capacity(expected),
                phase,
            },
            sync,
        },
        false => Start::Invalid,
    }
}

impl Receiver {
    /// Продвигает ровно одну фазу и оставляет непрочитанный pipeline во входе.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        match self.phase {
            Phase::Tail => self.consume_tail(input),
            Phase::Old | Phase::New => self.consume_literal(input),
        }
    }

    fn consume_literal(&mut self, input: &mut Vec<u8>) -> Progress {
        let remaining = self.expected.saturating_sub(self.bytes.len());
        let count = remaining.min(input.len());
        self.bytes.extend(input.drain(..count));
        if self.bytes.len() < self.expected {
            return Progress::Waiting;
        }
        let value = match String::from_utf8(std::mem::take(&mut self.bytes)) {
            Ok(value) => value,
            Err(_) => return Progress::InvalidUtf8,
        };
        match self.phase {
            Phase::Old => {
                self.old = value;
                self.phase = Phase::Tail;
                self.consume_tail(input)
            }
            Phase::New => Progress::Complete(Command::RenameScript {
                old: std::mem::take(&mut self.old),
                new: value,
            }),
            Phase::Tail => Progress::Invalid,
        }
    }

    fn consume_tail(&mut self, input: &mut Vec<u8>) -> Progress {
        let Some(end) = input.windows(2).position(|pair| pair == b"\r\n") else {
            let remaining = LIMIT.saturating_sub(self.bytes.len());
            let count = remaining.min(input.len());
            self.bytes.extend(input.drain(..count));
            return match self.bytes.len() < LIMIT {
                true => Progress::Waiting,
                false => Progress::Invalid,
            };
        };
        let framed = end.saturating_add(2);
        if framed > LIMIT.saturating_sub(self.bytes.len()) {
            return Progress::Invalid;
        }
        self.bytes.extend(input.drain(..framed));
        let tail = match String::from_utf8(std::mem::take(&mut self.bytes)) {
            Ok(value) => value,
            Err(_) => return Progress::InvalidUtf8,
        };
        match tail.strip_suffix("\r\n") {
            Some(value) => self.start_new(value.trim(), input),
            None => Progress::Invalid,
        }
    }

    fn start_new(&mut self, marker: &str, input: &mut Vec<u8>) -> Progress {
        match quoted(marker) {
            Some((new, "")) => Progress::Complete(Command::RenameScript {
                old: std::mem::take(&mut self.old),
                new,
            }),
            Some(_) => Progress::Invalid,
            None => self.start_literal(marker, input),
        }
    }

    fn start_literal(&mut self, marker: &str, input: &mut Vec<u8>) -> Progress {
        let Some((expected, sync)) = size(marker) else {
            return Progress::Invalid;
        };
        if expected > LIMIT {
            return Progress::Invalid;
        }
        self.phase = Phase::New;
        self.expected = expected;
        match sync {
            true => Progress::Continue,
            false => self.consume(input),
        }
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

#[cfg(test)]
#[path = "managesieve_rename_literal_tests.rs"]
mod tests;
