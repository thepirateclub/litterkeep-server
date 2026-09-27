//! Framing literal mechanism и дополнительных аргументов AUTHENTICATE.

use super::{LiteralReceiver, Operation, Progress};
use crate::managesieve::Command;

/// SASL-механизмы представлены вариантами, а не строковым состоянием FSM.
#[derive(Clone, Copy)]
pub(super) enum Mechanism {
    Plain,
    Unsupported,
}

impl Mechanism {
    /// Преобразует wire-строку механизма в закрытый тип сервера.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "PLAIN" => Self::Plain,
            _ => Self::Unsupported,
        }
    }
}

/// Получает механизм, затем продолжает разбор аргументов той же команды.
pub(super) fn consume(receiver: &mut LiteralReceiver, input: &mut Vec<u8>) -> Progress {
    let remaining = receiver.expected.saturating_sub(receiver.bytes.len());
    let consumed = remaining.min(input.len());
    receiver.bytes.extend(input.drain(..consumed));
    if receiver.bytes.len() < receiver.expected {
        return Progress::Waiting;
    }
    let mechanism = match String::from_utf8(std::mem::take(&mut receiver.bytes)) {
        Ok(value) => Mechanism::parse(&value),
        Err(_) => return Progress::InvalidUtf8,
    };
    consume_arguments(receiver, input, mechanism)
}

/// Читает остаток физической строки и принимает optional initial response.
fn consume_arguments(
    receiver: &mut LiteralReceiver,
    input: &mut Vec<u8>,
    mechanism: Mechanism,
) -> Progress {
    let Some(end) = input.windows(2).position(|pair| pair == b"\r\n") else {
        let remaining = super::MAX_LITERAL_BYTES.saturating_sub(receiver.bytes.len());
        let consumed = remaining.min(input.len());
        receiver.bytes.extend(input.drain(..consumed));
        return match receiver.bytes.len() < super::MAX_LITERAL_BYTES {
            true => Progress::Waiting,
            false => Progress::Invalid,
        };
    };
    let frame_length = end.saturating_add(2);
    if frame_length > super::MAX_LITERAL_BYTES {
        return Progress::Invalid;
    }
    let frame = input.drain(..frame_length).collect::<Vec<_>>();
    let arguments = match std::str::from_utf8(&frame) {
        Ok(value) => value.trim_end_matches("\r\n").trim(),
        Err(_) => return Progress::InvalidUtf8,
    };
    match arguments.is_empty() {
        true => empty_response(mechanism),
        false => response(receiver, input, arguments, mechanism),
    }
}

/// Выдаёт challenge только для распознанного механизма.
fn empty_response(mechanism: Mechanism) -> Progress {
    match mechanism {
        Mechanism::Plain => Progress::Complete(Command::AuthenticatePlain(None)),
        Mechanism::Unsupported => Progress::Complete(Command::Invalid),
    }
}

/// Выбирает обычный string response или следующий literal frame.
fn response(
    receiver: &mut LiteralReceiver,
    input: &mut Vec<u8>,
    arguments: &str,
    mechanism: Mechanism,
) -> Progress {
    match super::size(arguments) {
        Some((expected, continuation)) => match expected <= super::MAX_LITERAL_BYTES {
            true => start_response_literal(receiver, input, expected, continuation, mechanism),
            false => Progress::Invalid,
        },
        None => parse_quoted_response(arguments, mechanism),
    }
}

/// Переключает bounded receiver на literal SASL initial-response.
fn start_response_literal(
    receiver: &mut LiteralReceiver,
    input: &mut Vec<u8>,
    expected: usize,
    continuation: bool,
    mechanism: Mechanism,
) -> Progress {
    receiver.operation = match mechanism {
        Mechanism::Plain => Operation::AuthenticatePlain,
        Mechanism::Unsupported => Operation::AuthenticateRejected,
    };
    receiver.expected = expected;
    receiver.bytes.clear();
    receiver.terminator.clear();
    match continuation {
        true => Progress::NeedContinuation,
        false => receiver.consume(input),
    }
}

/// Передаёт response обратно штатному typed command parser-у.
fn parse_quoted_response(arguments: &str, mechanism: Mechanism) -> Progress {
    match mechanism {
        Mechanism::Plain => parse_plain_response(arguments),
        Mechanism::Unsupported => Progress::Complete(Command::Invalid),
    }
}

/// Передаёт поддерживаемый механизм обратно штатному typed command parser-у.
fn parse_plain_response(arguments: &str) -> Progress {
    let command = format!("AUTHENTICATE \"PLAIN\" {arguments}");
    match crate::managesieve::parse(&command) {
        Command::AuthenticatePlain(Some(response)) => {
            Progress::Complete(Command::AuthenticatePlain(Some(response)))
        }
        _ => Progress::Invalid,
    }
}
