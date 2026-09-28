//! Проверки грамматики RFC 3461 в типизированных командах SMTP.

use super::*;

/// Отклоняет некорректные значения и повторение параметров DSN.
#[test]
fn rejects_invalid_or_duplicate_dsn_parameters() {
    let cases = [
        (
            "MAIL FROM:<alice@test> RET=FULL RET=HDRS\r\n",
            ParseError::Syntax,
        ),
        ("RCPT TO:<bob@test> NOTIFY\r\n", ParseError::Syntax),
        ("RCPT TO:<bob@test> NOTIFY=INVALID\r\n", ParseError::Syntax),
        (
            "RCPT TO:<bob@test> NOTIFY=NEVER,FAILURE\r\n",
            ParseError::Syntax,
        ),
        (
            "RCPT TO:<bob@test> NOTIFY=SUCCESS NOTIFY=FAILURE\r\n",
            ParseError::Syntax,
        ),
        (
            "RCPT TO:<bob@test> ORCPT=rfc822;bob+2btest\r\n",
            ParseError::Syntax,
        ),
    ];
    for (wire, expected) in cases {
        assert_eq!(parse(wire), Err(expected), "wire command: {wire}");
    }
}

/// Ограничение RFC 3461 включает имя и значение параметра ORCPT.
#[test]
fn enforces_the_entire_orcpt_parameter_octet_limit() {
    let maximum = format!("RCPT TO:<bob@test> ORCPT=rfc822;{}\r\n", "a".repeat(487));
    let overlong = format!("RCPT TO:<bob@test> ORCPT=rfc822;{}\r\n", "a".repeat(488));
    assert!(matches!(parse(&maximum), Ok(Command::RcptWithDsn(_))));
    assert_eq!(parse(&overlong), Err(ParseError::Syntax));
}

/// Сохраняет корректные RFC 3461 поля, не выдавая их за поддержку сервером.
#[test]
fn parses_mail_and_recipient_dsn_parameters_as_typed_commands() {
    assert_eq!(
        parse("MAIL FROM:<alice@test> RET=full ENVID=job+2B42\r\n"),
        Ok(Command::MailWithDsn(
            MailFrom {
                reverse_path: "alice@test".into(),
                declared_size: None,
                body_mode: BodyMode::SevenBit,
                smtp_utf8: false,
            },
            MailDsn {
                return_content: Some(ReturnContent::Full),
                envelope_id: Some(b"job+42".to_vec()),
            },
        ))
    );
    assert_eq!(
        parse("RCPT TO:<bob@test> NOTIFY=FAILURE,success ORCPT=rfc822;bob+40test\r\n"),
        Ok(Command::RcptWithDsn(RcptTo {
            address: "bob@test".into(),
            dsn: RecipientDsn {
                notify: Some(NotifyPolicy::Conditions(
                    [NotifyCondition::Failure, NotifyCondition::Success]
                        .into_iter()
                        .collect(),
                )),
                original_recipient: Some(OriginalRecipient {
                    address_type: OriginalAddressType::Rfc822,
                    address: b"bob@test".to_vec(),
                }),
            },
        }))
    );
}
