//! Framing bounded literal-имени команды HAVESPACE.

use crate::managesieve::Command;

const MAX_NAME_BYTES: usize = 65_536;
const MAX_NUMERIC_TAIL: usize = 32;

pub(super) enum Start {
    NotLiteral,
    Invalid,
    Begin { receiver: Receiver, sync: bool },
}

pub(super) enum Progress {
    Waiting,
    Invalid,
    InvalidUtf8,
    Complete(Command),
}

pub(super) struct Receiver {
    expected: usize,
    name: Vec<u8>,
    tail: Vec<u8>,
}

/// Разбирает только literal в позиции имени, оставляя обычный quoted-путь парсеру.
pub(super) fn start(tail: &str) -> Start {
    match literal_size(tail.trim()) {
        Some((expected, sync)) if expected <= MAX_NAME_BYTES => Start::Begin {
            receiver: Receiver {
                expected,
                name: Vec::with_capacity(expected),
                tail: Vec::new(),
            },
            sync,
        },
        Some(_) => Start::Invalid,
        None => Start::NotLiteral,
    }
}

impl Receiver {
    /// Принимает ровно declared name octets, затем разбирает число до CRLF.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        if self.name.len() < self.expected {
            let remaining = self.expected.saturating_sub(self.name.len());
            let count = remaining.min(input.len());
            self.name.extend(input.drain(..count));
            if self.name.len() < self.expected {
                return Progress::Waiting;
            }
        }
        self.consume_size_tail(input)
    }

    fn consume_size_tail(&mut self, input: &mut Vec<u8>) -> Progress {
        let Some(end) = input.windows(2).position(|pair| pair == b"\r\n") else {
            let remaining = MAX_NUMERIC_TAIL.saturating_sub(self.tail.len());
            let count = remaining.min(input.len());
            self.tail.extend(input.drain(..count));
            return match self.tail.len() < MAX_NUMERIC_TAIL {
                true => Progress::Waiting,
                false => Progress::Invalid,
            };
        };
        let framed = end.saturating_add(2);
        if framed > MAX_NUMERIC_TAIL.saturating_sub(self.tail.len()) {
            return Progress::Invalid;
        }
        self.tail.extend(input.drain(..framed));
        self.complete()
    }

    fn complete(&mut self) -> Progress {
        let name = match String::from_utf8(std::mem::take(&mut self.name)) {
            Ok(value) => value,
            Err(_) => return Progress::InvalidUtf8,
        };
        let tail = match std::str::from_utf8(&self.tail) {
            Ok(value) => value,
            Err(_) => return Progress::InvalidUtf8,
        };
        let Some(number) = tail.strip_suffix("\r\n") else {
            return Progress::Invalid;
        };
        let Some(number) = number.strip_prefix(' ') else {
            return Progress::Invalid;
        };
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return Progress::Invalid;
        }
        match number.parse::<u64>() {
            Ok(size) => Progress::Complete(Command::HaveSpace { name, size }),
            Err(_) => Progress::Invalid,
        }
    }
}

fn literal_size(marker: &str) -> Option<(usize, bool)> {
    let body = marker.strip_prefix('{')?.strip_suffix('}')?;
    match body.strip_suffix('+') {
        Some(number) => decimal(number).map(|size| (size, false)),
        None => decimal(body).map(|size| (size, true)),
    }
}

fn decimal(value: &str) -> Option<usize> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
#[path = "managesieve_havespace_literal_tests.rs"]
mod tests;
