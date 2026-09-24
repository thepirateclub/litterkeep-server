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
