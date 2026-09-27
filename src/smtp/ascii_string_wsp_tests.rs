//! Регрессии ASCII-грамматики строковых SMTP-команд.

use super::{ParseError, parse};

/// RFC 5321 command strings допускают только printable ASCII, а не Unicode trim.
#[test]
fn rejects_unicode_whitespace_in_ascii_string_commands() {
    for wire in ["NOOP \u{00a0}\r\n", "HELP \u{00a0}\r\n"] {
        assert_eq!(
            parse(wire),
            Err(ParseError::Syntax),
            "wire command: {wire:?}"
        );
    }
}
