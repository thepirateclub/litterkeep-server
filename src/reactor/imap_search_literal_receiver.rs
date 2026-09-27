//! Владелец bounded-потока IMAP literal-ов и остатка команды.

use super::{LiteralCommand, MAX_IMAP_LITERAL, MAX_NON_SYNCHRONIZING_LITERAL};
use crate::imap::literal::{self, Marker as LiteralMarker, Mode as LiteralMode};

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
pub(in crate::reactor) struct Receiver {
    prefix: Vec<u8>,
    tag: String,
    expected: usize,
    bytes: Vec<u8>,
    literals: Vec<Vec<u8>>,
    remainder: Vec<u8>,
    phase: Phase,
    total_octets: usize,
    command: LiteralCommand,
}

/// Инкрементальный результат одного неблокирующего шага.
pub(in crate::reactor) enum Progress {
    Waiting,
    Continue(LiteralMode),
    Complete(Vec<Vec<u8>>),
    TooBig(String),
    InvalidString,
    InvalidSyntax,
}

impl Receiver {
    /// Принимает literal октетами, затем разбирает остаток либо следующий marker.
    pub(in crate::reactor) fn consume(&mut self, input: &mut Vec<u8>) -> Progress {
        match self.phase {
            Phase::Literal => match self.receive_literal(input) {
                Some(progress) => return progress,
                None => self.phase = Phase::CommandRemainder,
            },
            Phase::CommandRemainder => {}
        }
        self.receive_remainder(input)
    }

    /// Создаёт bounded-состояние для первого literal в принятой команде.
    pub(in crate::reactor) fn new(
        prefix: Vec<u8>,
        tag: String,
        marker: LiteralMarker,
        command: LiteralCommand,
    ) -> Option<Self> {
        let initial_size = match command {
            LiteralCommand::Append => prefix.len(),
            LiteralCommand::Login | LiteralCommand::Search | LiteralCommand::Mailbox => {
                prefix.len().checked_add(marker.size)?
            }
        };
        if initial_size > super::super::MAX_INPUT {
            return None;
        }
        Some(Self {
            prefix,
            tag,
            expected: marker.size,
            bytes: Vec::with_capacity(marker.size),
            literals: Vec::new(),
            remainder: Vec::new(),
            phase: Phase::Literal,
            total_octets: marker.size,
            command,
        })
    }

    /// Поглощает только объявленные octets; NUL запрещён в IMAP string literal.
    fn receive_literal(&mut self, input: &mut Vec<u8>) -> Option<Progress> {
        let remaining = self.expected.saturating_sub(self.bytes.len());
        let consumed = remaining.min(input.len());
        self.bytes.extend(input.drain(..consumed));
        if self.bytes.len() < self.expected {
            return Some(Progress::Waiting);
        }
        if self.bytes.contains(&0) && !matches!(self.command, LiteralCommand::Append) {
            return Some(Progress::InvalidString);
        }
        let value = std::mem::take(&mut self.bytes);
        self.prefix.push(0);
        self.literals.push(value);
        None
    }

    /// Выбирает завершение команды либо admission следующего literal-маркера.
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
            Some(marker) => {
                let Some(prefix) = self.remainder.get(..marker.prefix_size).map(<[u8]>::to_vec)
                else {
                    return Progress::InvalidSyntax;
                };
                self.next_literal(&prefix, marker.marker)
            }
            None => self.finish_command(input),
        }
    }

    /// Собирает ограниченную остаточную строку, не поглощая следующую команду.
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
        if next_size > super::super::MAX_INPUT {
            return RemainderProgress::LimitExceeded;
        }
        self.remainder.extend(input.drain(..available));
        match self.remainder.windows(2).any(|pair| pair == b"\r\n") {
            true => RemainderProgress::Complete,
            false => RemainderProgress::Waiting,
        }
    }

    /// Продолжает сборку после admission очередного sync или non-sync marker.
    fn next_literal(&mut self, before_marker: &[u8], marker: LiteralMarker) -> Progress {
        let Some(total) = self.total_octets.checked_add(marker.size) else {
            return Progress::TooBig(self.tag.clone());
        };
        let Some(command_size) = self
            .prefix
            .len()
            .checked_add(before_marker.len())
            .and_then(|size| size.checked_add(total))
        else {
            return Progress::TooBig(self.tag.clone());
        };
        let within_non_sync_limit = match marker.mode {
            LiteralMode::Synchronizing => true,
            LiteralMode::NonSynchronizing => marker.size <= MAX_NON_SYNCHRONIZING_LITERAL,
        };
        let maximum_literal = literal_limit(self.command);
        let total_within_limit = match self.command {
            LiteralCommand::Append => total <= maximum_literal,
            LiteralCommand::Login | LiteralCommand::Search | LiteralCommand::Mailbox => {
                command_size <= super::super::MAX_INPUT
            }
        };
        match (
            marker.size <= maximum_literal,
            within_non_sync_limit,
            total_within_limit,
        ) {
            (true, true, true) => {}
            _ => return Progress::TooBig(self.tag.clone()),
        }
        self.prefix.extend_from_slice(before_marker);
        self.remainder.clear();
        self.expected = marker.size;
        self.total_octets = total;
        self.bytes = Vec::with_capacity(marker.size);
        self.phase = Phase::Literal;
        Progress::Continue(marker.mode)
    }

    /// Завершает команду, возвращая лишние pipelined octets во входной буфер.
    fn finish_command(&mut self, input: &mut Vec<u8>) -> Progress {
        self.prefix.extend_from_slice(&self.remainder);
        self.remainder.clear();
        let mut rebuilt = std::mem::take(&mut self.prefix);
        rebuilt.extend(std::mem::take(input));
        *input = rebuilt;
        Progress::Complete(std::mem::take(&mut self.literals))
    }
}

fn literal_limit(command: LiteralCommand) -> usize {
    match command {
        LiteralCommand::Append => crate::smtp::MAX_MESSAGE_SIZE as usize,
        LiteralCommand::Login | LiteralCommand::Search | LiteralCommand::Mailbox => {
            MAX_IMAP_LITERAL
        }
    }
}

struct LiteralMarkerWithPrefix {
    prefix_size: usize,
    marker: LiteralMarker,
}

/// Распознаёт marker только в конце CRLF-строки, после последнего SP.
fn trailing_marker(content: &str) -> Option<LiteralMarkerWithPrefix> {
    let boundary = content.rfind(char::is_whitespace)?;
    let prefix_size = boundary.checked_add(1)?;
    let marker = content.get(boundary..)?.trim();
    let marker = literal::parse(marker)?;
    Some(LiteralMarkerWithPrefix {
        prefix_size,
        marker,
    })
}
