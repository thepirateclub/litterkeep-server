//! Unit-проверки evaluator-а IMAP SEARCH отдельно от production-кода.

use super::*;
use crate::mail_date::MailDate;
use crate::mail_flags::MessageFlags;
use std::path::PathBuf;

fn position(recent: &std::collections::BTreeSet<u64>) -> MatchPosition<'_> {
    MatchPosition {
        sequence: 3,
        maximum_sequence: 5,
        maximum_uid: 12,
        recent,
    }
}

fn message(bytes: &[u8]) -> crate::store::Message {
    crate::store::Message {
        uid: 9,
        path: PathBuf::new(),
        bytes: bytes.to_vec(),
        flags: MessageFlags::default(),
        internal_date: MailDate::from_unix_seconds(0),
    }
}

#[test]
fn matches_headers_body_text_size_and_conjunction() {
    let value = message(
        b"From: Alice <a@example.test>\r\nSubject: Report\r\nX-Case: Value\r\n\r\nBody TOKEN",
    );
    let query = SearchKey::And(vec![
        SearchKey::Header(HeaderSelector::Known(HeaderName::From), "ALICE".into()),
        SearchKey::Header(HeaderSelector::Other("x-case".into()), "value".into()),
        SearchKey::Body("token".into()),
        SearchKey::Larger(20),
    ]);
    let document = crate::mime::search_text::SearchDocument::parse(&value.bytes, true)
        .expect("search document");
    let recent = std::collections::BTreeSet::new();
    assert!(matches_key(
        &value,
        &query,
        Some(&document),
        &position(&recent)
    ));
    assert!(!matches_key(
        &value,
        &SearchKey::Smaller(20),
        None,
        &position(&recent)
    ));
    assert!(matches_key(
        &value,
        &SearchKey::Text("subject".into()),
        Some(&document),
        &position(&recent)
    ));
}

#[test]
fn matches_internal_date_and_boolean_tree() {
    let mut value = message(b"Subject: report\r\n\r\nbody");
    value.internal_date = MailDate::parse("2-Feb-2026 12:00:00 +0000").expect("date");
    let boundary = crate::imap::search_date::SearchDate::parse("2-Feb-2026").expect("date");
    let query = SearchKey::Or(
        Box::new(SearchKey::Before(boundary)),
        Box::new(SearchKey::Not(Box::new(SearchKey::Seen))),
    );
    let recent = std::collections::BTreeSet::new();
    assert!(matches_key(&value, &query, None, &position(&recent)));
    assert!(matches_key(
        &value,
        &SearchKey::On(boundary),
        None,
        &position(&recent)
    ));
    assert!(matches_key(
        &value,
        &SearchKey::Since(boundary),
        None,
        &position(&recent)
    ));
    assert!(!matches_key(
        &value,
        &SearchKey::Before(boundary),
        None,
        &position(&recent)
    ));
}

#[test]
fn matches_persistent_flags_and_session_recent_state() {
    use crate::mail_flags::SystemFlag;

    let mut value = message(b"Subject: report\r\n\r\nbody");
    value.flags = MessageFlags::from_flags([SystemFlag::Answered, SystemFlag::Flagged]);
    let recent = std::collections::BTreeSet::from([value.uid]);
    for key in [
        SearchKey::Answered,
        SearchKey::Flagged,
        SearchKey::Undraft,
        SearchKey::Recent,
        SearchKey::New,
    ] {
        assert!(matches_key(&value, &key, None, &position(&recent)));
    }
    assert!(!matches_key(
        &value,
        &SearchKey::Old,
        None,
        &position(&recent)
    ));
}

#[test]
fn matches_sequence_and_uid_sets_in_their_own_spaces() {
    let recent = std::collections::BTreeSet::new();
    let value = message(b"Subject: report\r\n\r\nbody");
    let sequences = crate::imap::sequence_set::SequenceSet::parse("3:4").expect("set");
    let uids = crate::imap::sequence_set::SequenceSet::parse("8:10").expect("set");
    assert!(matches_key(
        &value,
        &SearchKey::SequenceSet(sequences),
        None,
        &position(&recent)
    ));
    assert!(matches_key(
        &value,
        &SearchKey::UidSet(uids),
        None,
        &position(&recent)
    ));
}

#[test]
fn boolean_header_query_requests_header_parse() {
    let query = SearchKey::Not(Box::new(SearchKey::Or(
        Box::new(SearchKey::Header(
            HeaderSelector::Known(HeaderName::Subject),
            "ignored".into(),
        )),
        Box::new(SearchKey::Body("ignored".into())),
    )));
    assert!(requires_headers(&query));
}

#[test]
fn matches_sent_date_without_falling_back_to_internal_date() {
    let recent = std::collections::BTreeSet::new();
    let value = message(b"Date: Mon, 2 Feb 2026 23:00:00 +0000\r\n\r\nbody");
    let document = crate::mime::search_text::SearchDocument::parse(&value.bytes, false)
        .expect("search document");
    let boundary = crate::imap::search_date::SearchDate::parse("2-Feb-2026").expect("date");
    assert!(matches_key(
        &value,
        &SearchKey::SentOn(boundary),
        Some(&document),
        &position(&recent)
    ));
    let missing = message(b"Subject: no date\r\n\r\nbody");
    let missing_document = crate::mime::search_text::SearchDocument::parse(&missing.bytes, false)
        .expect("search document");
    assert!(!matches_key(
        &missing,
        &SearchKey::SentSince(boundary),
        Some(&missing_document),
        &position(&recent)
    ));
}
