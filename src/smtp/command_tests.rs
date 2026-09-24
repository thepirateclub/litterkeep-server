//! Изолированные проверки грамматики SMTP-команд и параметров MAIL.
use super::*;

#[test]
fn parses_size_parameter_as_typed_mail_metadata() {
    assert_eq!(
        parse("mail FROM:<alice@test> size=37\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: Some(37),
            body_mode: BodyMode::SevenBit,
        }))
    );
}

/// Разбирает RFC 6152 body modes и поддерживает совместный SIZE.
#[test]
fn parses_8bitmime_body_mode_and_parameter_order() {
    assert_eq!(
        parse("MAIL FROM:<alice@test> BODY=8BITMIME SIZE=23\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: Some(23),
            body_mode: BodyMode::EightBitMime,
        }))
    );
    assert_eq!(
        parse("MAIL FROM:<alice@test> BODY=7BIT\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
        }))
    );
    assert_eq!(
        parse("MAIL FROM:<alice@test> BODY=8BITMIME BODY=7BIT\r\n"),
        Err(ParseError::Syntax)
    );
}

/// Отклоняет некорректные значения, повторы и не рекламируемые параметры.
#[test]
fn distinguishes_invalid_size_from_unsupported_parameter() {
    let cases = [
        ("MAIL FROM:<alice@test> SIZE=\r\n", ParseError::Syntax),
        ("MAIL FROM:<alice@test> SIZE=+1\r\n", ParseError::Syntax),
        (
            "MAIL FROM:<alice@test> SIZE=123456789012345678901\r\n",
            ParseError::Syntax,
        ),
        (
            "MAIL FROM:<alice@test> SIZE=1 SIZE=2\r\n",
            ParseError::Syntax,
        ),
        ("MAIL FROM:<alice@test> BODY=FOO\r\n", ParseError::Syntax),
        (
            "RCPT TO:<bob@test> NOTIFY=SUCCESS\r\n",
            ParseError::UnsupportedParameter,
        ),
    ];
    for (wire, expected) in cases {
        assert_eq!(parse(wire), Err(expected), "wire command: {wire}");
    }
}

#[test]
fn accepts_empty_reverse_path_but_requires_recipient_path() {
    assert_eq!(
        parse("MAIL FROM:<>\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: String::new(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
        }))
    );
    assert_eq!(
        parse("RCPT TO:<bob@test>\r\n"),
        Ok(Command::Rcpt("bob@test".into()))
    );
    assert_eq!(parse("RCPT TO:<>\r\n"), Err(ParseError::Syntax));
}

/// Проверяет включение угловых скобок в RFC 5321 path limit.
#[test]
fn enforces_256_octet_reverse_and_forward_paths() {
    let maximum_domain = format!("{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(61));
    let overlong_domain = format!("{}.{}.{}", "a".repeat(63), "b".repeat(63), "c".repeat(62));
    let local_part = "u".repeat(64);
    let maximum_path = format!("{local_part}@{maximum_domain}");
    let overlong_path = format!("{local_part}@{overlong_domain}");

    assert_eq!(maximum_path.len() + 2, MAX_PATH_SIZE);
    assert!(matches!(
        parse(&format!("MAIL FROM:<{maximum_path}>\r\n")),
        Ok(Command::Mail(_))
    ));
    assert_eq!(
        parse(&format!("MAIL FROM:<{overlong_path}>\r\n")),
        Err(ParseError::Syntax)
    );
    assert!(matches!(
        parse(&format!("RCPT TO:<{maximum_path}>\r\n")),
        Ok(Command::Rcpt(_))
    ));
    assert_eq!(
        parse(&format!("RCPT TO:<{overlong_path}>\r\n")),
        Err(ParseError::Syntax)
    );
}

/// Проверяет 64-октетный local-part в обоих видах SMTP path.
#[test]
fn enforces_64_octet_local_parts() {
    let maximum_mailbox = format!("{}@test", "u".repeat(MAX_LOCAL_PART_SIZE));
    let overlong_mailbox = format!("{}@test", "u".repeat(MAX_LOCAL_PART_SIZE + 1));

    assert!(matches!(
        parse(&format!("MAIL FROM:<{maximum_mailbox}>\r\n")),
        Ok(Command::Mail(_))
    ));
    assert_eq!(
        parse(&format!("MAIL FROM:<{overlong_mailbox}>\r\n")),
        Err(ParseError::Syntax)
    );
    assert!(matches!(
        parse(&format!("RCPT TO:<{maximum_mailbox}>\r\n")),
        Ok(Command::Rcpt(_))
    ));
    assert_eq!(
        parse(&format!("RCPT TO:<{overlong_mailbox}>\r\n")),
        Err(ParseError::Syntax)
    );
}

/// Применяет ограниченную mailbox грамматику одинаково к MAIL и RCPT.
#[test]
fn rejects_malformed_mailbox_in_both_paths() {
    for mailbox in [
        "@test",
        ".alice@test",
        "alice.@test",
        "a..b@test",
        "a@-bad.test",
    ] {
        assert!(
            parse(&format!("MAIL FROM:<{mailbox}>\r\n")).is_err(),
            "MAIL mailbox: {mailbox}"
        );
        assert!(
            parse(&format!("RCPT TO:<{mailbox}>\r\n")).is_err(),
            "RCPT mailbox: {mailbox}"
        );
    }
}

#[test]
fn parses_supported_commands_and_auth_mechanism() {
    assert_eq!(
        parse("AUTH PLAIN AGJvYgB0ZXN0LXBhc3N3b3Jk\r\n"),
        Ok(Command::AuthPlain("AGJvYgB0ZXN0LXBhc3N3b3Jk".into()))
    );
    assert_eq!(parse("STARTTLS\r\n"), Ok(Command::StartTls));
    assert_eq!(parse("NOOP\r\n"), Ok(Command::Noop(None)));
    assert_eq!(
        parse("NOOP keep state\r\n"),
        Ok(Command::Noop(Some("keep state".into())))
    );
    assert_eq!(
        parse("VRFY alice@test\r\n"),
        Ok(Command::Vrfy("alice@test".into()))
    );
    assert_eq!(parse("EXPN team\r\n"), Ok(Command::Expn("team".into())));
    assert_eq!(parse("HELP\r\n"), Ok(Command::Help(None)));
    assert_eq!(
        parse("HELO sender.test\r\n"),
        Ok(Command::Helo("sender.test".into()))
    );
    assert_eq!(
        parse("AUTH LOGIN response\r\n"),
        Err(ParseError::UnsupportedAuthenticationMechanism)
    );
}
