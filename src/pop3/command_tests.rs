//! Проверяет POP3 command grammar, printable ASCII и RFC argument limits.
use super::*;
use crate::pop3::{LanguageRequest, SaslInitialResponse};

#[test]
fn accepts_user_argument_at_rfc1939_character_limit() {
    let username = "u".repeat(40);
    let wire = format!("USER {username}\r\n");
    assert_eq!(parse(&wire), Command::User(username));
    let password = "p".repeat(40);
    assert_eq!(
        parse(&format!("PASS {password}\r\n")),
        Command::Pass(password)
    );
}

#[test]
fn rejects_user_and_pass_arguments_over_rfc1939_limit() {
    let username = format!("USER {}\r\n", "u".repeat(41));
    let password = format!("PASS {}\r\n", "p".repeat(41));
    assert_eq!(parse(&username), Command::Invalid);
    assert_eq!(parse(&password), Command::Invalid);
}

/// RFC 6856 credentials считают символы, а общий POP3 предел считает октеты.
#[test]
fn accepts_multibyte_credentials_by_character_count() {
    let username = "ю".repeat(40);
    let user_wire = format!("USER {username}\r\n");
    assert!(user_wire.len() > username.chars().count());
    assert_eq!(parse(&user_wire), Command::User(username));

    let password = "密碼🔐".repeat(10);
    assert_eq!(password.chars().count(), 30);
    assert_eq!(
        parse(&format!("PASS {password}\r\n")),
        Command::Pass(password)
    );
}

/// Прочие команды остаются ASCII; credentials по-прежнему ограничены 40 символами.
#[test]
fn rejects_non_ascii_outside_credentials_and_credential_overflow() {
    assert_eq!(parse("STAT café\r\n"), Command::Invalid);
    assert_eq!(
        parse(&format!("USER {}\r\n", "ю".repeat(41))),
        Command::Invalid
    );
    assert_eq!(
        parse(&format!("PASS {}\r\n", "密".repeat(41))),
        Command::Invalid
    );
}

#[test]
fn accepts_command_at_rfc2449_octet_limit_for_sasl_payload() {
    let token = "a".repeat(242);
    let wire = format!("AUTH PLAIN {token}\r\n");
    assert_eq!(wire.len(), MAX_COMMAND_LINE_SIZE);
    assert!(matches!(parse(&wire), Command::Auth { .. }));
}

#[test]
fn recognizes_only_the_scram_sha1_sasl_mechanism() {
    assert!(matches!(
        parse("AUTH SCRAM-SHA-1\r\n"),
        Command::Auth {
            mechanism: super::SaslMechanism::ScramSha1,
            initial_response: None,
        }
    ));
    assert!(matches!(
        parse("AUTH scram-sha-1 =\r\n"),
        Command::Auth {
            mechanism: super::SaslMechanism::ScramSha1,
            initial_response: Some(SaslInitialResponse::Empty),
        }
    ));
}

#[test]
fn rejects_command_over_rfc2449_octet_limit() {
    let token = "a".repeat(243);
    let wire = format!("AUTH PLAIN {token}\r\n");
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
fn pass_keeps_spaces_inside_its_single_password_argument() {
    assert_eq!(
        parse("PASS two words\r\n"),
        Command::Pass("two words".into())
    );
    assert_eq!(
        parse("PASS trailing space \r\n"),
        Command::Pass("trailing space ".into())
    );
    assert_eq!(
        parse("PASS  leading space\r\n"),
        Command::Pass(" leading space".into())
    );
    assert_eq!(parse("PASS \r\n"), Command::Invalid);
    assert_eq!(
        parse(&format!("PASS {}\r\n", "p".repeat(41))),
        Command::Invalid
    );
    assert_eq!(parse("USER \r\n"), Command::Invalid);
    assert_eq!(parse("USER two words\r\n"), Command::Invalid);
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
            initial_response: Some(SaslInitialResponse::Encoded("AGJvYgB0ZXN0".into())),
        }
    );
    assert_eq!(
        parse("AUTH PLAIN =\r\n"),
        Command::Auth {
            mechanism: SaslMechanism::Plain,
            initial_response: Some(SaslInitialResponse::Empty),
        }
    );
    assert_eq!(
        parse("AUTH SCRAM-SHA-256 token\r\n"),
        Command::Auth {
            mechanism: SaslMechanism::ScramSha256,
            initial_response: Some(SaslInitialResponse::Encoded("token".into())),
        }
    );
    assert_eq!(
        parse("AUTH SCRAM-SHA-256-PLUS token\r\n"),
        Command::Auth {
            mechanism: SaslMechanism::ScramSha256Plus,
            initial_response: Some(SaslInitialResponse::Encoded("token".into())),
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
