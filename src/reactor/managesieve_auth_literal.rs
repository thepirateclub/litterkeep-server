//! Распознавание SASL initial-response literal в ManageSieve AUTHENTICATE.

use super::{LiteralReceiver, Operation, Progress, Start, literal, quoted_then_tail, size};

/// Поддерживаемые здесь SASL-механизмы после разбора протокольной строки.
enum Mechanism {
    Plain,
    Unsupported,
}

impl Mechanism {
    /// Преобразует внешнее имя механизма в типизированный вариант.
    fn parse(value: &str) -> Self {
        match value.eq_ignore_ascii_case("PLAIN") {
            true => Self::Plain,
            false => Self::Unsupported,
        }
    }
}

/// Возвращает receiver только для quoted PLAIN mechanism и literal response.
pub(super) fn start(tail: &str) -> Start {
    let Some((mechanism, marker)) = quoted_then_tail(tail) else {
        return Start::NotLiteral;
    };
    match Mechanism::parse(&mechanism) {
        Mechanism::Plain => literal(Operation::AuthenticatePlain, marker),
        Mechanism::Unsupported => Start::NotLiteral,
    }
}

/// Преобразует initial-response octets в типизированную команду PLAIN.
pub(super) fn initial_response(response: String) -> Progress {
    Progress::Complete(crate::managesieve::Command::AuthenticatePlain(Some(
        response,
    )))
}

/// Преобразует continuation octets в типизированный ответ SASL.
pub(super) fn challenge_response(response: String) -> Progress {
    Progress::Complete(crate::managesieve::Command::AuthenticateResponse(response))
}

/// Строит bounded receiver для literal ответа после SASL challenge.
pub fn start_response(line: &[u8]) -> Start {
    let Ok(text) = std::str::from_utf8(line) else {
        return Start::Invalid;
    };
    let Some(text) = text.strip_suffix("\r\n") else {
        return Start::Invalid;
    };
    let Some((expected, continuation)) = size(text.trim()) else {
        return Start::NotLiteral;
    };
    match expected <= super::MAX_LITERAL_BYTES {
        true => Start::Begin {
            receiver: LiteralReceiver::new(Operation::AuthenticateResponse, expected),
            continuation,
        },
        false => Start::Invalid,
    }
}
