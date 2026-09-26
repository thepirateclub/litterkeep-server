//! Граничные проверки SEARCH literal framing и восстановления команды.

use super::{Progress, Receiver, Start, start};
use crate::imap::literal::{Marker, Mode};

fn new_receiver(prefix: &[u8], size: usize) -> Receiver {
    Receiver::new(
        prefix.to_vec(),
        "a".to_owned(),
        Marker {
            size,
            mode: Mode::Synchronizing,
        },
    )
    .expect("bounded literal")
}

#[test]
fn detects_login_search_and_uid_search_synchronizing_literals() {
    assert!(matches!(start(b"a LOGIN {3}\r\n"), Start::Begin { .. }));
    assert!(matches!(start(b"a LOGIN bob {8}\r\n"), Start::Begin { .. }));
    assert!(matches!(
        start(b"a SEARCH BODY {5}\r\n"),
        Start::Begin { .. }
    ));
    assert!(matches!(
        start(b"b uid search HEADER {0}\r\n"),
        Start::Begin { .. }
    ));
    for command in [
        b"c CREATE {8}\r\n".as_slice(),
        b"d SELECT {8}\r\n",
        b"e LIST \"\" {8}\r\n",
        b"f RENAME {8} {7}\r\n",
        b"g UID COPY 1 {8}\r\n",
    ] {
        assert!(matches!(start(command), Start::Begin { .. }));
    }
    assert!(matches!(
        start(b"h SEARCH SUBJECT {5+}\r\n"),
        Start::Begin { .. }
    ));
    assert!(matches!(
        start(b"i SEARCH SUBJECT {5+}\r\n"),
        Start::Begin {
            mode: Mode::NonSynchronizing,
            ..
        }
    ));
}

#[test]
fn literal_minus_enforces_4096_octets_and_closes_on_non_sync_overflow() {
    assert!(matches!(
        start(b"a SEARCH BODY {4097+}\r\n"),
        Start::Reject {
            terminate: true,
            ..
        }
    ));
    assert!(matches!(
        start(b"a SEARCH BODY {65537}\r\n"),
        Start::Reject {
            terminate: false,
            ..
        }
    ));
}

#[test]
/// Два клиента literal становятся двумя обязательными typed LOGIN аргументами.
fn login_literals_rebuild_into_the_typed_login_command() {
    let mut receiver = new_receiver(b"a LOGIN ", 3);
    let mut input = b"bob {12}\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::Continue(_)
    ));
    let mut password = b"bob-password\r\n".to_vec();
    let Progress::Complete(values) = receiver.consume(&mut password) else {
        panic!("LOGIN literals должны завершить команду");
    };
    assert_eq!(values, vec![b"bob".to_vec(), b"bob-password".to_vec()]);
    assert_eq!(password, b"a LOGIN \0 \0\r\n");
    assert_eq!(
        crate::imap::parse_literals_with_mode(
            std::str::from_utf8(&password).expect("internal command marker"),
            values,
            crate::imap::tokenizer::StringMode::Legacy,
        )
        .command,
        crate::imap::Command::Login {
            username: "bob".into(),
            password: b"bob-password".to_vec(),
        }
    );
}

#[test]
fn leaves_other_commands_and_quoted_braces_unchanged() {
    assert!(matches!(
        start(b"a APPEND INBOX {5}\r\n"),
        Start::NotLiteral
    ));
    assert!(matches!(
        start(b"a SEARCH BODY \"{5}\"\r\n"),
        Start::NotLiteral
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
        let mut receiver = new_receiver(prefix, literal.len());
        let mut input = literal.get(..split).expect("prefix split").to_vec();
        assert!(matches!(receiver.consume(&mut input), Progress::Waiting));
        input.extend(literal.get(split..).expect("suffix split"));
        input.extend_from_slice(suffix);
        let Progress::Complete(values) = receiver.consume(&mut input) else {
            panic!("SEARCH literal должна завершиться");
        };
        assert_eq!(values, vec![literal.to_vec()]);
        assert_eq!(input, b"a SEARCH BODY \0 TEXT tail\r\n");
    }
}

#[test]
fn utf8_search_literal_is_not_reparsed_as_wire_quoted_text() {
    let literal = "Привет".as_bytes();
    let mut receiver = new_receiver(b"a SEARCH SUBJECT ", literal.len());
    let mut input = literal.to_vec();
    input.extend_from_slice(b"\r\n");
    let Progress::Complete(values) = receiver.consume(&mut input) else {
        panic!("UTF-8 SEARCH literal должна завершиться");
    };
    assert_eq!(values, vec!["Привет".as_bytes().to_vec()]);
    assert_eq!(input, b"a SEARCH SUBJECT \0\r\n");
    assert_eq!(
        crate::imap::parse_literals_with_mode(
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
            charset_explicit: false,
            result: crate::imap::search_result::ResultRequest::Legacy,
        }
    );
}

#[test]
fn receives_multiple_literals_and_preserves_pipelined_command() {
    let mut receiver = new_receiver(b"a SEARCH HEADER ", 7);
    let mut first = b"Subject {7+}\r".to_vec();
    assert!(matches!(receiver.consume(&mut first), Progress::Waiting));
    first.extend_from_slice(b"\n");
    assert!(matches!(
        receiver.consume(&mut first),
        Progress::Continue(Mode::NonSynchronizing)
    ));
    assert!(first.is_empty());
    let mut second = b"literal\r\nNEXT NOOP\r\n".to_vec();
    let Progress::Complete(values) = receiver.consume(&mut second) else {
        panic!("оба SEARCH literals должны завершить команду");
    };
    assert_eq!(values, vec![b"Subject".to_vec(), b"literal".to_vec()]);
    assert_eq!(second, b"a SEARCH HEADER \0 \0\r\nNEXT NOOP\r\n");
}

#[test]
fn rejects_an_oversized_followup_non_sync_literal_before_allocation() {
    let mut receiver = new_receiver(b"a SEARCH HEADER ", 1);
    let mut input = b"x {4097+}\r\n".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::TooBig(tag) if tag == "a"
    ));
}

#[test]
fn synchronizing_literal_preserves_embedded_crlf_octets() {
    let literal = b"alpha\r\nbeta";
    let mut receiver = new_receiver(b"a LOGIN ", literal.len());
    let mut input = literal.to_vec();
    input.extend_from_slice(b"\r\n");
    let Progress::Complete(values) = receiver.consume(&mut input) else {
        panic!("CRLF внутри literal не должны завершать его раньше счётчика октетов");
    };
    assert_eq!(values, vec![b"alpha\r\nbeta".to_vec()]);
    assert_eq!(input, b"a LOGIN \0\r\n");
}

#[test]
fn preserves_non_utf8_literal_octets_and_rejects_nul() {
    let mut receiver = new_receiver(b"a LOGIN bob ", 1);
    let mut input = b"\xff\r\n".to_vec();
    let Progress::Complete(values) = receiver.consume(&mut input) else {
        panic!("не-UTF-8 парольный octet должен сохраниться как literal");
    };
    assert_eq!(values, vec![b"\xff".to_vec()]);
    assert_eq!(
        crate::imap::parse_literals_with_mode(
            std::str::from_utf8(&input).expect("internal command marker"),
            values,
            crate::imap::tokenizer::StringMode::Legacy,
        )
        .command,
        crate::imap::Command::Login {
            username: "bob".into(),
            password: b"\xff".to_vec(),
        }
    );

    let mut receiver = new_receiver(b"a LOGIN bob ", 2);
    let mut input = b"a\0".to_vec();
    assert!(matches!(
        receiver.consume(&mut input),
        Progress::InvalidString
    ));
}
