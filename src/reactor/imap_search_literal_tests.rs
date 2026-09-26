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
        let mut receiver = Receiver::new(prefix.to_vec(), literal.len()).expect("bounded literal");
        let mut input = literal.get(..split).expect("prefix split").to_vec();
        assert!(matches!(receiver.consume(&mut input), Progress::Waiting));
        input.extend(literal.get(split..).expect("suffix split"));
        input.extend_from_slice(suffix);
        let Progress::Complete(values) = receiver.consume(&mut input) else {
            panic!("SEARCH literal должна завершиться");
        };
        assert_eq!(values, vec![String::from_utf8(literal.to_vec()).unwrap()]);
        assert_eq!(input, b"a SEARCH BODY \0 TEXT tail\r\n");
    }
}

#[test]
fn utf8_search_literal_is_not_reparsed_as_wire_quoted_text() {
    let literal = "Привет".as_bytes();
    let mut receiver =
        Receiver::new(b"a SEARCH SUBJECT ".to_vec(), literal.len()).expect("bounded literal");
    let mut input = literal.to_vec();
    input.extend_from_slice(b"\r\n");
    let Progress::Complete(values) = receiver.consume(&mut input) else {
        panic!("UTF-8 SEARCH literal должна завершиться");
    };
    assert_eq!(values, vec!["Привет"]);
    assert_eq!(input, b"a SEARCH SUBJECT \0\r\n");
    assert_eq!(
        crate::imap::parse_search_literals_with_mode(
            std::str::from_utf8(&input).expect("internal command marker"),
            values,
            crate::imap::tokenizer::StringMode::Legacy,
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
fn receives_multiple_literals_and_preserves_pipelined_command() {
    let mut receiver = Receiver::new(b"a SEARCH HEADER ".to_vec(), 7).expect("bounded literal");
    let mut first = b"Subject {7}\r".to_vec();
    assert!(matches!(receiver.consume(&mut first), Progress::Waiting));
    first.extend_from_slice(b"\n");
    assert!(matches!(receiver.consume(&mut first), Progress::Continue));
    assert!(first.is_empty());
    let mut second = b"literal\r\nNEXT NOOP\r\n".to_vec();
    let Progress::Complete(values) = receiver.consume(&mut second) else {
        panic!("оба SEARCH literals должны завершить команду");
    };
    assert_eq!(values, vec!["Subject", "literal"]);
    assert_eq!(second, b"a SEARCH HEADER \0 \0\r\nNEXT NOOP\r\n");
}

#[test]
fn rejects_invalid_utf8_and_line_breaks_in_string_literals() {
    for literal in [b"\xff".as_slice(), b"a\r"] {
        let mut receiver =
            Receiver::new(b"a SEARCH BODY ".to_vec(), literal.len()).expect("bounded literal");
        let mut input = literal.to_vec();
        assert!(matches!(
            receiver.consume(&mut input),
            Progress::InvalidString
        ));
    }
}
