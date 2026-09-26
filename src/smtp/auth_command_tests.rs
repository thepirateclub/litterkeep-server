//! Unit-проверки AUTH PLAIN grammar с и без initial response.
use super::super::{Command, ParseError, parse};

#[test]
/// Optional response can arrive with AUTH or after the empty server challenge.
fn accepts_plain_with_optional_initial_response() {
    assert_eq!(parse("AUTH PLAIN\r\n"), Ok(Command::AuthPlain(None)));
    assert_eq!(
        parse("AUTH PLAIN AGJvYgB0ZXN0\r\n"),
        Ok(Command::AuthPlain(Some("AGJvYgB0ZXN0".into())))
    );
}

#[test]
/// Empty, tab-separated, and repeated-space forms are rejected as syntax.
fn rejects_empty_or_whitespace_contaminated_initial_response() {
    for wire in [
        "AUTH PLAIN \r\n",
        "AUTH PLAIN  AGJvYgB0ZXN0\r\n",
        "AUTH PLAIN AGJvYgB0ZXN0\t\r\n",
        "AUTH PLAIN\tAGJvYgB0ZXN0\r\n",
    ] {
        assert_eq!(
            parse(wire),
            Err(ParseError::Syntax),
            "wire command: {wire:?}"
        );
    }
    assert_eq!(
        parse("AUTH LOGIN\r\n"),
        Err(ParseError::UnsupportedAuthenticationMechanism)
    );
}
