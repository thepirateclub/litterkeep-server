//! Проверяет POP3 command grammar, printable ASCII и RFC 2449 длины.
use super::*;
use crate::pop3::LanguageRequest;

#[test]
fn accepts_command_at_rfc2449_octet_limit() {
    let username = "u".repeat(248);
    let wire = format!("USER {username}\r\n");
    assert_eq!(wire.len(), MAX_COMMAND_LINE_SIZE);
    assert_eq!(parse(&wire), Command::User(username));
}

#[test]
fn rejects_command_over_rfc2449_octet_limit() {
    let wire = format!("USER {}\r\n", "u".repeat(249));
    assert_eq!(wire.len(), MAX_COMMAND_LINE_SIZE + 1);
    assert_eq!(parse(&wire), Command::Invalid);
}

#[test]
fn rejects_non_printable_non_ascii_and_noncanonical_separators() {
    for wire in [
        "USER  bob\r\n",
        "USER\tbob\r\n",
        "USER bob \r\n",
        "USER b\u{0001}ob\r\n",
        "USER bоб\r\n",
        "US bob\r\n",
        "USER bob\n",
    ] {
        assert_eq!(parse(wire), Command::Invalid, "wire command: {wire:?}");
    }
}

#[test]
fn preserves_normal_command_arity() {
    assert_eq!(parse("STAT\r\n"), Command::Stat);
    assert_eq!(parse("USER bob\r\n"), Command::User("bob".into()));
    assert_eq!(
        parse("TOP 2 3\r\n"),
        Command::Top {
            number: 2,
            lines: 3
        }
    );
    assert_eq!(parse("UTF8\r\n"), Command::Utf8);
    assert_eq!(parse("UTF8 extra\r\n"), Command::Invalid);
    assert_eq!(parse("LANG\r\n"), Command::Lang(LanguageRequest::Listing));
    assert_eq!(
        parse("LANG EN\r\n"),
        Command::Lang(LanguageRequest::English)
    );
    assert_eq!(
        parse("LANG *\r\n"),
        Command::Lang(LanguageRequest::Preferred)
    );
    assert_eq!(
        parse("LANG de\r\n"),
        Command::Lang(LanguageRequest::Unsupported)
    );
    assert_eq!(parse("LANG en--US\r\n"), Command::Invalid);
    assert_eq!(parse("NOOP extra\r\n"), Command::Invalid);
}

#[test]
fn parses_sasl_auth_plain_with_or_without_initial_response() {
    assert_eq!(
        parse("AUTH plain\r\n"),
        Command::Auth {
            mechanism: SaslMechanism::Plain,
            initial_response: None,
        }
    );
    assert_eq!(
        parse("AUTH PLAIN AGJvYgB0ZXN0\r\n"),
        Command::Auth {
            mechanism: SaslMechanism::Plain,
            initial_response: Some("AGJvYgB0ZXN0".into()),
        }
    );
    assert_eq!(
        parse("AUTH SCRAM-SHA-256 token\r\n"),
        Command::Auth {
            mechanism: SaslMechanism::Unsupported,
            initial_response: Some("token".into()),
        }
    );
}

#[test]
fn rejects_malformed_sasl_auth_arity_and_empty_initial_response() {
    for wire in [
        "AUTH\r\n",
        "AUTH  PLAIN\r\n",
        "AUTH PLAIN \r\n",
        "AUTH PLAIN token extra\r\n",
    ] {
        assert_eq!(parse(wire), Command::Invalid, "wire command: {wire:?}");
    }
}
