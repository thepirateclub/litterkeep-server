//! Проверки синтаксиса и точного сохранения SMTP peer diagnostic.

use super::{ReplyExpectation, ReplyResult, expect_with_diagnostic, parse_reply, read_reply_line};
use crate::outbound::DeliveryOutcome;
use std::io::{Cursor, ErrorKind};

#[test]
fn accepts_reply_line_at_the_512_octet_boundary() {
    let mut wire = b"250 ".to_vec();
    wire.extend(std::iter::repeat_n(b'x', 506));
    wire.extend_from_slice(b"\r\n");
    assert_eq!(wire.len(), 512);
    let mut reader = Cursor::new(wire.clone());
    assert_eq!(read_reply_line(&mut reader).expect("reply line").len(), 512);
    assert_eq!(reader.position(), 512);
}

#[test]
fn rejects_reply_line_one_octet_over_the_512_octet_boundary() {
    let mut wire = b"250 ".to_vec();
    wire.extend(std::iter::repeat_n(b'x', 507));
    wire.extend_from_slice(b"\r\n");
    let mut reader = Cursor::new(wire);
    assert_eq!(
        read_reply_line(&mut reader)
            .expect_err("oversized reply")
            .kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn rejects_reply_line_without_crlf_terminator() {
    let mut reader = Cursor::new(b"250 peer\n");
    assert_eq!(
        read_reply_line(&mut reader)
            .expect_err("bare LF reply")
            .kind(),
        ErrorKind::InvalidData
    );
}

/// RFC 5321 §4.2 требует принимать код без текста и с пустым текстом.
#[test]
fn accepts_reply_code_without_text_or_with_only_separator() {
    assert_eq!(parse_reply("250\r\n").expect("bare code"), (250, false));
    assert_eq!(parse_reply("250 \r\n").expect("empty text"), (250, false));
    assert_eq!(parse_reply("250 text\r\n").expect("text"), (250, false));
    assert_eq!(
        parse_reply("250-\r\n").expect("empty continuation"),
        (250, true)
    );
}

/// RFC 5321 §4.2 требует три ASCII-цифры и полный CRLF framing.
#[test]
fn rejects_non_numeric_code_and_missing_crlf() {
    assert_eq!(
        parse_reply("+25 text\r\n")
            .expect_err("sign is not a reply digit")
            .kind(),
        ErrorKind::InvalidData
    );
    assert_eq!(
        parse_reply("250 text\n")
            .expect_err("bare LF is not a complete reply line")
            .kind(),
        ErrorKind::InvalidData
    );
}

#[test]
fn failure_result_retains_multiline_code_and_bounded_text() {
    let mut reader = Cursor::new(b"550-5.1.1 first line\r\n550 mailbox missing\r\n");
    let (result, diagnostic) = expect_with_diagnostic(&mut reader, ReplyExpectation::Recipient)
        .expect("complete negative reply");
    assert_eq!(
        result,
        ReplyResult::Rejected(DeliveryOutcome::PermanentFailure)
    );
    assert_eq!(
        diagnostic.map(|value| (value.code, value.text)),
        Some((550, "5.1.1 first line mailbox missing".into()))
    );
}

#[test]
fn temporary_replies_retain_code_and_closing_421_is_diagnostic() {
    let mut temporary = Cursor::new(b"451 4.3.0 try later\r\n");
    let (result, diagnostic) = expect_with_diagnostic(&mut temporary, ReplyExpectation::Recipient)
        .expect("temporary failure");
    assert_eq!(result, ReplyResult::Rejected(DeliveryOutcome::Deferred));
    assert_eq!(diagnostic.map(|value| value.code), Some(451));

    let mut closing = Cursor::new(b"421 4.4.2 closing\r\n");
    let (result, diagnostic) = expect_with_diagnostic(&mut closing, ReplyExpectation::Recipient)
        .expect("closing response");
    assert_eq!(result, ReplyResult::Closing);
    assert_eq!(diagnostic.map(|value| value.code), Some(421));
}
