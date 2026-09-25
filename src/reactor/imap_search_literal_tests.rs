//! Граничные проверки SEARCH literal framing и восстановления команды.

use super::{Progress, Receiver, Start, start};

#[test]
fn detects_search_and_uid_search_synchronizing_literals() {
    assert!(matches!(start(b"a SEARCH BODY {5}\r\n"), Start::Begin(_)));
    assert!(matches!(
        start(b"b uid search HEADER {0}\r\n"),
        Start::Begin(_)
    ));
}

#[test]
fn leaves_other_commands_and_quoted_braces_unchanged() {
    assert!(matches!(
        start(b"a APPEND INBOX {5}\r\n"),
        Start::NotSearchLiteral
    ));
    assert!(matches!(
        start(b"a SEARCH BODY \"{5}\"\r\n"),
        Start::NotSearchLiteral
    ));
}

#[test]
fn rejects_literals_over_the_search_budget_before_continuation() {
    assert!(matches!(
        start(b"a SEARCH BODY {65537}\r\n"),
        Start::Reject { .. }
    ));
}

#[test]
fn restores_escaped_literal_at_every_tcp_split() {
    let literal = b"alpha\"\\omega";
    let suffix = b" TEXT tail\r\n";
    let prefix = b"a SEARCH BODY ";
    for split in 0..literal.len() {
        let mut receiver = Receiver {
            prefix: prefix.to_vec(),
            expected: literal.len(),
            bytes: Vec::new(),
        };
        let mut input = literal.get(..split).expect("prefix split").to_vec();
        assert!(matches!(receiver.consume(&mut input), Progress::Waiting));
        input.extend(literal.get(split..).expect("suffix split"));
        input.extend_from_slice(suffix);
        let Progress::Complete(value) = receiver.consume(&mut input) else {
            panic!("SEARCH literal должна завершиться");
        };
        assert_eq!(value.as_bytes(), literal);
        assert_eq!(input, b"a SEARCH BODY \0 TEXT tail\r\n");
    }
}

#[test]
fn utf8_search_literal_is_not_reparsed_as_wire_quoted_text() {
    let literal = "Привет".as_bytes();
    let mut receiver = Receiver {
        prefix: b"a SEARCH SUBJECT ".to_vec(),
        expected: literal.len(),
        bytes: Vec::new(),
    };
    let mut input = literal.to_vec();
    let Progress::Complete(value) = receiver.consume(&mut input) else {
        panic!("UTF-8 SEARCH literal должна завершиться");
    };
    assert_eq!(value, "Привет");
    assert_eq!(input, b"a SEARCH SUBJECT \0");
    assert_eq!(
        crate::imap::parse_search_literal(
            std::str::from_utf8(&input).expect("internal command marker"),
            &value,
        )
        .command,
        crate::imap::Command::Search {
            addressing: crate::imap::Addressing::Sequence,
            key: crate::imap::search::SearchKey::Envelope(
                crate::mime::header_block::HeaderName::Subject,
                "Привет".into(),
            ),
            charset: crate::imap::search_charset::Charset::Utf8,
            result: crate::imap::search_result::ResultRequest::Legacy,
        }
    );
}

#[test]
fn rejects_invalid_utf8_and_line_breaks_in_string_literals() {
    for literal in [b"\xff".as_slice(), b"a\r"] {
        let mut receiver = Receiver {
            prefix: b"a SEARCH BODY ".to_vec(),
            expected: literal.len(),
            bytes: Vec::new(),
        };
        let mut input = literal.to_vec();
        assert!(matches!(
            receiver.consume(&mut input),
            Progress::InvalidString
        ));
    }
}
