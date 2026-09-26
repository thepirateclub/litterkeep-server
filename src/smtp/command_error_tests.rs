//! Регрессии для различения неизвестной команды и неверного аргумента.
use super::super::{Command, ParseError, parse};

/// RFC 5321 отделяет неизвестный verb от ошибки параметров известной команды.
#[test]
fn classifies_unknown_command_separately_from_invalid_arguments() {
    assert_eq!(parse("WUT\r\n"), Err(ParseError::UnrecognizedCommand));
    assert_eq!(parse("WUT extra\r\n"), Err(ParseError::UnrecognizedCommand));
    assert_eq!(parse("DATA unexpected\r\n"), Err(ParseError::Syntax));
    assert_eq!(parse("DATA\tunexpected\r\n"), Err(ParseError::Syntax));
    assert_eq!(parse("NOOP\r\n"), Ok(Command::Noop(None)));
}
