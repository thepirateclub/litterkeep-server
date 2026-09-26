//! Ограниченный RFC synchronizing literal framing для IMAP-команд.

/// Решение до выдачи continuation клиенту.
pub(super) enum Start {
    NotLiteral,
    Begin(Receiver),
    Reject { tag: String, reason: &'static str },
}

const MAX_IMAP_LITERAL: usize = 64 * 1024;

/// Семейство команды определяет, допустим ли literal в последнем аргументе.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiteralCommand {
    Login,
    Search,
    Mailbox,
}

/// Ищет sync-literal в строковых и mailbox аргументах поддержанных команд.
pub(super) fn start(line: &[u8]) -> Start {
    let Ok(command) = std::str::from_utf8(line) else {
        return Start::NotLiteral;
    };
    let Some(command) = command.strip_suffix("\r\n") else {
        return Start::NotLiteral;
    };
    let Some((prefix, tag, size)) = initial_marker(command) else {
        return Start::NotLiteral;
    };
    match size <= MAX_IMAP_LITERAL {
        true => match Receiver::new(prefix.as_bytes().to_vec(), size) {
            Some(receiver) => Start::Begin(receiver),
            None => Start::Reject {
                tag: tag.to_owned(),
                reason: "IMAP command exceeds limit",
            },
        },
        false => Start::Reject {
            tag: tag.to_owned(),
            reason: "IMAP literal exceeds limit",
        },
    }
}

fn initial_marker(command: &str) -> Option<(&str, &str, usize)> {
    let boundary = command.rfind(|character: char| character.is_ascii_whitespace())?;
    let prefix_end = boundary.checked_add(1)?;
    let prefix = command.get(..prefix_end)?;
    let marker = command.get(boundary..)?.trim();
    let tag = literal_tag(prefix)?;
    Some((prefix, tag, literal_size(marker)?))
}

fn literal_tag(prefix: &str) -> Option<&str> {
    let mut words = prefix.split_ascii_whitespace();
    let tag = words.next()?;
    let first = words.next()?.to_ascii_uppercase();
    let _recognized_command = match first.as_str() {
        "LOGIN" => LiteralCommand::Login,
        "SEARCH" => LiteralCommand::Search,
        "UID" => match words.next().map(str::to_ascii_uppercase).as_deref() {
            Some("SEARCH") => LiteralCommand::Search,
            Some("COPY" | "MOVE") => LiteralCommand::Mailbox,
            _ => return None,
        },
        "CREATE" | "DELETE" | "RENAME" | "SUBSCRIBE" | "UNSUBSCRIBE" | "LIST" | "LSUB"
        | "STATUS" | "COPY" | "MOVE" | "SELECT" | "EXAMINE" => LiteralCommand::Mailbox,
        _ => return None,
    };
    Some(tag)
}

fn literal_size(marker: &str) -> Option<usize> {
    let digits = marker.strip_prefix('{')?.strip_suffix('}')?;
    match digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        true => None,
        false => digits.parse().ok(),
    }
}

/// Фаза приёма literal или остатка команды после его octets.
enum Phase {
    Literal,
    CommandRemainder,
}

/// Итог накопления командного хвоста до CRLF.
enum RemainderProgress {
    Waiting,
    Complete,
    LimitExceeded,
}

/// Владеет всей командой с несколькими literals, не удерживая worker.
pub(super) struct Receiver {
    prefix: Vec<u8>,
    expected: usize,
    bytes: Vec<u8>,
    literals: Vec<String>,
    remainder: Vec<u8>,
    phase: Phase,
    total_octets: usize,
}

/// Инкрементальный результат одного неблокирующего шага.
pub(super) enum Progress {
    Waiting,
    Continue,
    Complete(Vec<String>),
    InvalidString,
    InvalidSyntax,
}

impl Receiver {
    /// Принимает literal октетами, затем разбирает продолжение команды или следующий marker.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        match self.phase {
            Phase::Literal => match self.receive_literal(input) {
                Some(progress) => return progress,
                None => self.phase = Phase::CommandRemainder,
            },
            Phase::CommandRemainder => {}
        }
        self.receive_remainder(input)
    }

    /// Создаёт ограниченное состояние для первого sync-literal команды.
    fn new(prefix: Vec<u8>, expected: usize) -> Option<Self> {
        if prefix.len().checked_add(expected)? > super::MAX_INPUT {
            return None;
        }
        Some(Self {
            prefix,
            expected,
            bytes: Vec::with_capacity(expected),
            literals: Vec::new(),
            remainder: Vec::new(),
            phase: Phase::Literal,
            total_octets: expected,
        })
    }

    /// Поглощает ровно объявленные octets и хранит их как строковое значение.
    fn receive_literal(&mut self, input: &mut Vec<u8>) -> Option<Progress> {
        let remaining = self.expected.saturating_sub(self.bytes.len());
        let consumed = remaining.min(input.len());
        self.bytes.extend(input.drain(..consumed));
        if self.bytes.len() < self.expected {
            return Some(Progress::Waiting);
        }
        let Ok(value) = String::from_utf8(std::mem::take(&mut self.bytes)) else {
            return Some(Progress::InvalidString);
        };
        if value.contains('\0') {
            return Some(Progress::InvalidString);
        }
        self.prefix.push(0);
        self.literals.push(value);
        None
    }

    /// Определяет конец команды либо admission следующего sync-literal.
    fn receive_remainder(&mut self, input: &mut Vec<u8>) -> Progress {
        match self.collect_remainder(input) {
            RemainderProgress::Complete => {}
            RemainderProgress::Waiting => return Progress::Waiting,
            RemainderProgress::LimitExceeded => return Progress::InvalidSyntax,
        }
        let Some(line) = std::str::from_utf8(&self.remainder).ok() else {
            return Progress::InvalidSyntax;
        };
        let Some(content) = line.strip_suffix("\r\n") else {
            return Progress::InvalidSyntax;
        };
        match trailing_marker(content) {
            Some((prefix_size, size)) => {
                let Some(prefix) = self.remainder.get(..prefix_size).map(<[u8]>::to_vec) else {
                    return Progress::InvalidSyntax;
                };
                self.next_literal(&prefix, size)
            }
            None => self.finish_command(input),
        }
    }

    /// Собирает одну ограниченную остаточную строку, не поглощая pipelined command.
    fn collect_remainder(&mut self, input: &mut Vec<u8>) -> RemainderProgress {
        let old_end = self.remainder.windows(2).position(|pair| pair == b"\r\n");
        if old_end.is_some() {
            return RemainderProgress::Complete;
        }
        let new_end = input.windows(2).position(|pair| pair == b"\r\n");
        let available = match new_end {
            Some(end) => match end.checked_add(2) {
                Some(size) => size,
                None => return RemainderProgress::LimitExceeded,
            },
            None => input.len(),
        };
        let Some(next_size) = self
            .prefix
            .len()
            .checked_add(self.remainder.len())
            .and_then(|size| size.checked_add(available))
        else {
            return RemainderProgress::LimitExceeded;
        };
        if next_size > super::MAX_INPUT {
            return RemainderProgress::LimitExceeded;
        }
        self.remainder.extend(input.drain(..available));
        match self.remainder.windows(2).any(|pair| pair == b"\r\n") {
            true => RemainderProgress::Complete,
            false => RemainderProgress::Waiting,
        }
    }

    /// Продолжает протокольную команду после обнаружения очередного sync marker.
    fn next_literal(&mut self, before_marker: &[u8], expected: usize) -> Progress {
        let Some(total) = self.total_octets.checked_add(expected) else {
            return Progress::InvalidSyntax;
        };
        let Some(command_size) = self
            .prefix
            .len()
            .checked_add(before_marker.len())
            .and_then(|size| size.checked_add(total))
        else {
            return Progress::InvalidSyntax;
        };
        if expected > MAX_IMAP_LITERAL || command_size > super::MAX_INPUT {
            return Progress::InvalidSyntax;
        }
        self.prefix.extend_from_slice(before_marker);
        self.remainder.clear();
        self.expected = expected;
        self.total_octets = total;
        self.bytes = Vec::with_capacity(expected);
        self.phase = Phase::Literal;
        Progress::Continue
    }

    /// Завершает сборку, оставляя любые следующие команды во входном буфере.
    fn finish_command(&mut self, input: &mut Vec<u8>) -> Progress {
        self.prefix.extend_from_slice(&self.remainder);
        self.remainder.clear();
        let mut rebuilt = std::mem::take(&mut self.prefix);
        rebuilt.extend(std::mem::take(input));
        *input = rebuilt;
        Progress::Complete(std::mem::take(&mut self.literals))
    }
}

fn trailing_marker(content: &str) -> Option<(usize, usize)> {
    let boundary = content.rfind(char::is_whitespace)?;
    let prefix_size = boundary.checked_add(1)?;
    let marker = content.get(boundary..)?.trim();
    Some((prefix_size, literal_size(marker)?))
}

#[cfg(test)]
#[path = "imap_search_literal_tests.rs"]
mod tests;
