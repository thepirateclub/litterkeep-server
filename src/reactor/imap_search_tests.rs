//! Unit-проверки evaluator-а IMAP SEARCH отдельно от production-кода.

use super::*;
use crate::mail_date::MailDate;
use crate::mail_flags::MessageFlags;
use std::path::PathBuf;
use std::sync::OnceLock;

fn empty_saved_search() -> &'static std::collections::BTreeSet<u64> {
    static EMPTY: OnceLock<std::collections::BTreeSet<u64>> = OnceLock::new();
    EMPTY.get_or_init(std::collections::BTreeSet::new)
}

fn position(recent: &std::collections::BTreeSet<u64>) -> MatchPosition<'_> {
    MatchPosition {
        sequence: 3,
        maximum_sequence: 5,
        maximum_uid: 12,
        recent,
        saved_search_uids: empty_saved_search(),
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

/// Envelope searches omit RFC 5322 delimiters absent from ENVELOPE values.
#[test]
fn address_search_uses_decoded_envelope_fields_not_raw_headers() {
    let recent = std::collections::BTreeSet::new();
    let value = message(
        b"From: =?UTF-8?Q?Alice?= <alice@example.test>\r\n\
          To: Bob <bob@example.test>\r\n\
          Cc: Team <team@example.test>\r\n\
          Bcc: Hidden <hidden@example.test>\r\n\
          Subject: =?UTF-8?Q?Quarterly_report?=\r\n\r\nbody",
    );
    let document = crate::mime::search_text::SearchDocument::parse(&value.bytes, false)
        .expect("search document");
    for (field, needle) in [
        (HeaderName::From, "alice@example.test"),
        (HeaderName::To, "bob@example.test"),
        (HeaderName::Cc, "team@example.test"),
        (HeaderName::Bcc, "hidden@example.test"),
        (HeaderName::Subject, "quarterly report"),
    ] {
        assert!(
            matches_key(
                &value,
                &SearchKey::Envelope(field, needle.into()),
                Some(&document),
                &position(&recent)
            ),
            "field={field:?} needle={needle:?} projection matched={}",
            document.envelope_contains(field, needle)
        );
    }
    assert!(!matches_key(
        &value,
        &SearchKey::Envelope(HeaderName::From, "<alice".into()),
        Some(&document),
        &position(&recent)
    ));
}

#[test]
fn save_projection_obeys_esearch_min_and_max_selection() {
    let matches = vec![
        SearchMatch { number: 2, uid: 12 },
        SearchMatch { number: 4, uid: 14 },
        SearchMatch { number: 9, uid: 19 },
    ];
    let min_max = crate::imap::search_result::ResultRequest::Extended(vec![
        crate::imap::search_result::ReturnItem::Save,
        crate::imap::search_result::ReturnItem::Min,
        crate::imap::search_result::ReturnItem::Max,
    ]);
    assert_eq!(saved_uids(&min_max, &matches), [12, 19].into());
    let min_only = crate::imap::search_result::ResultRequest::Extended(vec![
        crate::imap::search_result::ReturnItem::Save,
        crate::imap::search_result::ReturnItem::Min,
    ]);
    assert_eq!(saved_uids(&min_only, &matches), [12].into());
}

#[test]
fn saved_search_key_matches_uids_independent_of_sequence_context() {
    let value = message(b"Subject: saved\r\n\r\nbody");
    let recent = std::collections::BTreeSet::new();
    let saved = std::collections::BTreeSet::from([value.uid]);
    let position = MatchPosition {
        sequence: 3,
        maximum_sequence: 5,
        maximum_uid: 12,
        recent: &recent,
        saved_search_uids: &saved,
    };
    assert!(matches_key(&value, &SearchKey::Saved, None, &position));
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
