//! Типизированный разбор команды SMTP AUTH и optional SASL initial response.
use super::{Command, ParseError};

/// Механизм из закрытого набора поддерживаемых сервером вариантов.
#[derive(Clone, Copy)]
enum Mechanism {
    Plain,
    Unsupported,
}

/// Разбирает AUTH PLAIN как с initial response, так и с continuation flow.
pub(super) fn parse(argument: &str) -> Result<Command, ParseError> {
    if argument.bytes().any(is_non_space_whitespace) {
        return Err(ParseError::Syntax);
    }
    let (mechanism, response) = match argument.split_once(' ') {
        Some((mechanism, response)) => (mechanism, Some(response)),
        None => (argument, None),
    };
    match parse_mechanism(mechanism) {
        Mechanism::Plain => parse_plain(response),
        Mechanism::Unsupported => Err(ParseError::UnsupportedAuthenticationMechanism),
    }
}

/// RFC command separators use SP; other ASCII whitespace is invalid framing.
fn is_non_space_whitespace(byte: u8) -> bool {
    match byte {
        b' ' => false,
        _ => byte.is_ascii_whitespace(),
    }
}

/// Сводит wire keyword к типизированному механизму до обработки его формы.
fn parse_mechanism(value: &str) -> Mechanism {
    match value.to_ascii_uppercase().as_str() {
        "PLAIN" => Mechanism::Plain,
        _ => Mechanism::Unsupported,
    }
}

/// Проверяет один initial-response token либо оставляет его на continuation.
fn parse_plain(response: Option<&str>) -> Result<Command, ParseError> {
    let Some(response) = response else {
        return Ok(Command::AuthPlain(None));
    };
    if response.is_empty() {
        return Err(ParseError::Syntax);
    }
    if response.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ParseError::Syntax);
    }
    Ok(Command::AuthPlain(Some(response.to_owned())))
}

#[cfg(test)]
#[path = "auth_command_tests.rs"]
mod tests;
