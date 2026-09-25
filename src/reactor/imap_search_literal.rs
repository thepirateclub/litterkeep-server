//! Ограниченный RFC synchronizing literal framing для SEARCH строк.

const MAX_SEARCH_LITERAL: usize = 64 * 1024;

/// Решение до выдачи continuation клиенту.
pub(super) enum Start {
    NotSearchLiteral,
    Begin(Receiver),
    Reject { tag: String, reason: &'static str },
}

/// Ищет sync-literal только в SEARCH/UID SEARCH и сохраняет его префикс.
pub(super) fn start(line: &[u8]) -> Start {
    let Ok(command) = std::str::from_utf8(line) else {
        return Start::NotSearchLiteral;
    };
    let Some(command) = command.strip_suffix("\r\n") else {
        return Start::NotSearchLiteral;
    };
    let Some(boundary) = command.rfind(|character: char| character.is_ascii_whitespace()) else {
        return Start::NotSearchLiteral;
    };
    let Some(prefix_end) = boundary.checked_add(1) else {
        return Start::NotSearchLiteral;
    };
    let Some(prefix) = command.get(..prefix_end) else {
        return Start::NotSearchLiteral;
    };
    let Some(marker) = command.get(boundary..).map(str::trim) else {
        return Start::NotSearchLiteral;
    };
    let Some(tag) = search_tag(prefix) else {
        return Start::NotSearchLiteral;
    };
    let Some(size) = literal_size(marker) else {
        return Start::NotSearchLiteral;
    };
    match size <= MAX_SEARCH_LITERAL {
        true => Start::Begin(Receiver {
            prefix: prefix.as_bytes().to_vec(),
            expected: size,
            bytes: Vec::with_capacity(size),
        }),
        false => Start::Reject {
            tag: tag.to_owned(),
            reason: "SEARCH literal exceeds limit",
        },
    }
}

fn search_tag(prefix: &str) -> Option<&str> {
    let mut words = prefix.split_ascii_whitespace();
    let tag = words.next()?;
    let first = words.next()?.to_ascii_uppercase();
    let search = match first.as_str() {
        "SEARCH" => true,
        "UID" => words.next()?.eq_ignore_ascii_case("SEARCH"),
        _ => false,
    };
    match search {
        true => Some(tag),
        false => None,
    }
}

fn literal_size(marker: &str) -> Option<usize> {
    let digits = marker.strip_prefix('{')?.strip_suffix('}')?;
    match digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        true => None,
        false => digits.parse().ok(),
    }
}

/// Владеет одной literal до полной сборки команды, не удерживая worker.
pub(super) struct Receiver {
    prefix: Vec<u8>,
    expected: usize,
    bytes: Vec<u8>,
}

/// Инкрементальный результат одного неблокирующего шага.
pub(super) enum Progress {
    Waiting,
    Complete(String),
    InvalidString,
}

impl Receiver {
    /// Поглощает ровно объявленное число октетов и восстанавливает parser input.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        let remaining = self.expected.saturating_sub(self.bytes.len());
        let consumed = remaining.min(input.len());
        self.bytes.extend(input.drain(..consumed));
        if self.bytes.len() < self.expected {
            return Progress::Waiting;
        }
        let Ok(value) = String::from_utf8(std::mem::take(&mut self.bytes)) else {
            return Progress::InvalidString;
        };
        if value.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
            return Progress::InvalidString;
        }
        let mut rebuilt = self.prefix.clone();
        rebuilt.push(0);
        rebuilt.extend(std::mem::take(input));
        *input = rebuilt;
        Progress::Complete(value)
    }
}

#[cfg(test)]
#[path = "imap_search_literal_tests.rs"]
mod tests;
