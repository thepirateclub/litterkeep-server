//! RFC 6531 VRFY/EXPN query и reply-capability регрессии.

use super::{Command, ParseError, ReplyEncoding, VerifyRequest, parse};

#[test]
fn parses_ascii_verify_without_utf8_reply_permission() {
    let expected = VerifyRequest {
        query: "alice@example.test".into(),
        reply_encoding: ReplyEncoding::AsciiOnly,
    };
    assert_eq!(
        parse("VRFY alice@example.test\r\n"),
        Ok(Command::Vrfy(expected))
    );
}

#[test]
fn parses_utf8_query_and_valueless_reply_permission_for_both_commands() {
    let expected = VerifyRequest {
        query: "почта@example.test".into(),
        reply_encoding: ReplyEncoding::Utf8Allowed,
    };
    assert_eq!(
        parse("VRFY почта@example.test SMTPUTF8\r\n"),
        Ok(Command::Vrfy(expected.clone()))
    );
    assert_eq!(
        parse("EXPN почта@example.test smtputf8\r\n"),
        Ok(Command::Expn(expected))
    );
    assert!(matches!(
        parse("VRFY name\u{00a0}value SMTPUTF8\r\n"),
        Ok(Command::Vrfy(_))
    ));
}

#[test]
fn utf8_query_does_not_require_permission_for_utf8_replies() {
    assert_eq!(
        parse("VRFY почта@example.test\r\n"),
        Ok(Command::Vrfy(VerifyRequest {
            query: "почта@example.test".into(),
            reply_encoding: ReplyEncoding::AsciiOnly,
        }))
    );
}

#[test]
fn rejects_smtputf8_parameter_values_and_invalid_strings() {
    for command in [
        "VRFY alice@example.test SMTPUTF8=YES\r\n",
        "EXPN alice@example.test SMTPUTF8=\r\n",
        "VRFY \r\n",
        "EXPN name\u{0085}with-control\r\n",
    ] {
        assert_eq!(parse(command), Err(ParseError::Syntax), "{command:?}");
    }
}
