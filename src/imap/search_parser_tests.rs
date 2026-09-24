//! Граничные проверки typed IMAP SEARCH parser-а.

use crate::imap::search::{HeaderSelector, SearchKey};
use crate::imap::{self, Command};
use crate::mime::header_block::HeaderName;

#[test]
fn parses_text_header_size_and_implicit_and() {
    let command = imap::parse("a SEARCH FROM alice SUBJECT report LARGER 12\r\n").command;
    let Command::Search { key, .. } = command else {
        panic!("search command");
    };
    assert!(matches!(key, SearchKey::And(keys) if keys.len() == 3));
}

/// Адресные criteria используют отдельные ENVELOPE projection variants.
#[test]
fn parses_envelope_and_raw_header_criteria_separately() {
    let command =
        imap::parse("a SEARCH SUBJECT report FROM alice TO bob CC team BCC hidden\r\n").command;
    let Command::Search {
        key: SearchKey::And(keys),
        ..
    } = command
    else {
        panic!("address criteria");
    };
    assert!(matches!(
        keys.first(),
        Some(SearchKey::Envelope(HeaderName::Subject, _))
    ));
    assert!(matches!(
        keys.get(1),
        Some(SearchKey::Envelope(HeaderName::From, _))
    ));
    assert!(matches!(
        keys.get(2),
        Some(SearchKey::Envelope(HeaderName::To, _))
    ));
    assert!(matches!(
        keys.get(3),
        Some(SearchKey::Envelope(HeaderName::Cc, _))
    ));
    assert!(matches!(
        keys.get(4),
        Some(SearchKey::Envelope(HeaderName::Bcc, _))
    ));
    assert!(matches!(
        imap::parse("a SEARCH HEADER SUBJECT report\r\n").command,
        Command::Search {
            key: SearchKey::Header(HeaderSelector::Known(HeaderName::Subject), _),
            ..
        }
    ));
}

#[test]
fn parses_keyword_search_keys_and_rejects_invalid_flags() {
    let command = imap::parse("a SEARCH KEYWORD $Junk UNKEYWORD CustomLabel\r\n").command;
    assert!(matches!(
        command,
        Command::Search {
            key: SearchKey::And(keys),
            ..
        } if keys.len() == 2
    ));
    assert_eq!(
        imap::parse("a SEARCH KEYWORD \\Unknown\r\n").command,
        Command::Unsupported
    );
}

/// Каждый базовый search-key RFC 9051 должен распознаваться отдельно.
#[test]
fn parses_every_base_rfc_9051_search_key() {
    let criteria = [
        "1:3",
        "ALL",
        "ANSWERED",
        "BCC hidden",
        "BEFORE 1-Feb-2026",
        "BODY term",
        "CC team",
        "DELETED",
        "DRAFT",
        "FLAGGED",
        "FROM alice",
        "HEADER SUBJECT term",
        "KEYWORD $Junk",
        "LARGER 10",
        "NEW",
        "NOT SEEN",
        "OLD",
        "ON 1-Feb-2026",
        "OR SEEN ANSWERED",
        "RECENT",
        "SEEN",
        "SENTBEFORE 1-Feb-2026",
        "SENTON 1-Feb-2026",
        "SENTSINCE 1-Feb-2026",
        "SINCE 1-Feb-2026",
        "SMALLER 10",
        "SUBJECT report",
        "TEXT term",
        "TO bob",
        "UID 1:3",
        "UNANSWERED",
        "UNDELETED",
        "UNDRAFT",
        "UNFLAGGED",
        "UNKEYWORD $Junk",
        "UNSEEN",
    ];

    for criterion in criteria {
        let command = format!("a SEARCH {criterion}\r\n");
        assert!(
            matches!(imap::parse(&command).command, Command::Search { .. }),
            "RFC 9051 search key was rejected: {criterion}"
        );
    }
}
