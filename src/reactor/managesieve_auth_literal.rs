//! Распознавание SASL initial-response literal в ManageSieve AUTHENTICATE.

use super::auth_mechanism::Mechanism;
use super::{LiteralReceiver, Operation, Progress, Start, literal, quoted_then_tail, size};

/// Выбирает receiver для quoted SASL mechanism и optional initial response.
pub(super) fn start(tail: &str) -> Start {
    match quoted_then_tail(tail) {
        Some((mechanism, marker)) => match Mechanism::parse(&mechanism) {
            Mechanism::Plain => literal(Operation::AuthenticatePlain, marker),
            Mechanism::ScramSha1 => literal(Operation::AuthenticateScramSha1, marker),
            Mechanism::ScramSha256 => literal(Operation::AuthenticateScramSha256, marker),
            Mechanism::Unsupported => Start::NotLiteral,
        },
        None => match size(tail.trim()) {
            Some((expected, continuation)) if expected <= super::MAX_LITERAL_BYTES => {
                Start::Begin {
                    receiver: LiteralReceiver::new(Operation::AuthenticateMechanism, expected),
                    continuation,
                }
            }
            Some(_) => Start::Invalid,
            None => Start::NotLiteral,
        },
    }
}

/// Преобразует initial-response octets в typed SCRAM command.
pub(super) fn initial_scram_response(response: String) -> Progress {
    Progress::Complete(crate::managesieve::Command::AuthenticateScramSha1(Some(
        response,
    )))
}

/// Преобразует initial-response octets SHA-256 в typed SCRAM command.
pub(super) fn sha256_response(response: String) -> Progress {
    Progress::Complete(crate::managesieve::Command::AuthenticateScramSha256(Some(
        response,
    )))
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
