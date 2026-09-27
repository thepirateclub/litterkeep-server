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
            smtp_utf8: false,
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
            smtp_utf8: false,
        }))
    );
    assert_eq!(
        parse("MAIL FROM:<alice@test> BODY=7BIT\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
            smtp_utf8: false,
        }))
    );
    assert_eq!(
        parse("MAIL FROM:<alice@test> BODY=8BITMIME BODY=7BIT\r\n"),
        Err(ParseError::Syntax)
    );
}

/// RFC 6531 SMTPUTF8 является флагом MAIL, а не параметром со значением.
#[test]
fn parses_smtputf8_flag_and_rejects_values_or_duplicates() {
    assert_eq!(
        parse("MAIL FROM:<alice@test> SMTPUTF8 BODY=8BITMIME\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: None,
            body_mode: BodyMode::EightBitMime,
            smtp_utf8: true,
        }))
    );
    for wire in [
        "MAIL FROM:<alice@test> SMTPUTF8=YES\r\n",
        "MAIL FROM:<alice@test> SMTPUTF8 SMTPUTF8\r\n",
    ] {
        assert_eq!(parse(wire), Err(ParseError::Syntax));
    }
    assert_eq!(
        parse("MAIL FROM:<alice@test> SMTPUTF8\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@test".into(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
            smtp_utf8: true,
        }))
    );
}

#[test]
fn utf8_mailbox_requires_mail_parameter_but_not_eight_bit_body_mode() {
    assert_eq!(
        parse("MAIL FROM:<алиса@litterkeep.test> SMTPUTF8\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "алиса@litterkeep.test".into(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
            smtp_utf8: true,
        }))
    );
    assert_eq!(
        parse("MAIL FROM:<алиса@litterkeep.test>\r\n"),
        Err(ParseError::SmtpUtf8Required)
    );
}

#[test]
fn smtp_utf8_mail_line_has_rfc6531_ten_octet_extension() {
    assert_eq!(MAX_SMTPUTF8_MAIL_LINE_SIZE, 522);
    assert_eq!(command_line_limit(b"MAIL FROM:<a@test> SMTPUTF8\r\n"), 522);
    assert_eq!(command_line_limit(b"NOOP\r\n"), 512);
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
fn quoted_local_part_can_contain_path_terminator_and_at_sign() {
    assert_eq!(
        parse("RCPT TO:<\"a>b\\@c\"@test>\r\n"),
        Ok(Command::Rcpt("\"a>b\\@c\"@test".into()))
    );
}

#[test]
fn source_route_is_accepted_and_ignored_for_both_path_kinds() {
    assert_eq!(
        parse("MAIL FROM:<@source.test,@relay.test:\"alice\"@test>\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "\"alice\"@test".into(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
            smtp_utf8: false,
        }))
    );
    assert_eq!(
        parse("RCPT TO:<@source.test:\"bob\"@test>\r\n"),
        Ok(Command::Rcpt("\"bob\"@test".into()))
    );
    assert_eq!(
        parse("RCPT TO:<@source.test,,@relay.test:bob@test>\r\n"),
        Err(ParseError::Syntax)
    );
}

#[test]
fn parses_address_literals_in_both_mailbox_path_kinds() {
    assert_eq!(
        parse("MAIL FROM:<alice@[192.0.2.1]>\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: "alice@[192.0.2.1]".into(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
            smtp_utf8: false,
        }))
    );
    assert_eq!(
        parse("RCPT TO:<\"bob\"@[IPv6:2001:db8::1]>\r\n"),
        Ok(Command::Rcpt("\"bob\"@[IPv6:2001:db8::1]".into()))
    );
    assert_eq!(
        parse("RCPT TO:<bob@[256.0.0.1]>\r\n"),
        Err(ParseError::Syntax)
    );
}

#[test]
fn accepts_empty_reverse_path_but_requires_recipient_path() {
    assert_eq!(
        parse("MAIL FROM:<>\r\n"),
        Ok(Command::Mail(MailFrom {
            reverse_path: String::new(),
            declared_size: None,
            body_mode: BodyMode::SevenBit,
            smtp_utf8: false,
        }))
    );
    assert_eq!(
        parse("RCPT TO:<bob@test>\r\n"),
        Ok(Command::Rcpt("bob@test".into()))
    );
    assert_eq!(parse("RCPT TO:<>\r\n"), Err(ParseError::Syntax));
    for recipient in ["Postmaster", "POSTMASTER", "postmaster"] {
        assert_eq!(
            parse(&format!("RCPT TO:<{recipient}>\r\n")),
            Ok(Command::Rcpt(recipient.to_owned()))
        );
    }
    assert_eq!(parse("MAIL FROM:<Postmaster>\r\n"), Err(ParseError::Syntax));
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
fn parses_non_transaction_control_commands() {
    assert_eq!(parse("STARTTLS\r\n"), Ok(Command::StartTls));
    assert_eq!(parse("NOOP\r\n"), Ok(Command::Noop(None)));
    assert_eq!(
        parse("NOOP keep state\r\n"),
        Ok(Command::Noop(Some("keep state".into())))
    );
    assert_eq!(parse("HELP\r\n"), Ok(Command::Help(None)));
}
