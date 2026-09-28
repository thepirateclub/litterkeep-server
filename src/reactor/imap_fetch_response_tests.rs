//! Проверяет RFC 9051 UID item при сериализации FETCH-ответов.

use super::*;
use crate::mail_date::MailDate;
use crate::store::Message;
use std::{collections::BTreeSet, path::PathBuf};

fn message(uid: u64) -> Message {
    Message {
        uid,
        path: PathBuf::new(),
        bytes: b"Subject: uid response\r\n\r\nbody".to_vec(),
        flags: crate::mail_flags::MessageFlags::default(),
        keywords: BTreeSet::new(),
        internal_date: MailDate::from_unix_seconds(0),
    }
}

fn uid_item_count(response: &[u8], uid: u64) -> usize {
    let item = format!("UID {uid}");
    response
        .windows(item.len())
        .filter(|window| *window == item.as_bytes())
        .count()
}

fn flags_item_count(response: &[u8]) -> usize {
    response
        .windows(b"FLAGS (".len())
        .filter(|window| *window == b"FLAGS (")
        .count()
}

#[test]
fn uid_fetch_includes_uid_when_client_does_not_request_it() {
    let response = prepare(
        1,
        &message(73),
        &[imap::FetchAttribute::Flags],
        imap::Addressing::Uid,
        &BTreeSet::new(),
    )
    .expect("UID FETCH response");

    assert_eq!(uid_item_count(&response, 73), 1);
}

#[test]
fn uid_fetch_suppresses_duplicate_requested_uid_items() {
    let response = prepare(
        1,
        &message(73),
        &[
            imap::FetchAttribute::Uid,
            imap::FetchAttribute::Flags,
            imap::FetchAttribute::Uid,
        ],
        imap::Addressing::Uid,
        &BTreeSet::new(),
    )
    .expect("UID FETCH response");

    assert_eq!(uid_item_count(&response, 73), 1);
}

#[test]
fn sequence_fetch_does_not_inject_unrequested_uid() {
    let response = prepare(
        1,
        &message(73),
        &[imap::FetchAttribute::Flags],
        imap::Addressing::Sequence,
        &BTreeSet::new(),
    )
    .expect("FETCH response");

    assert_eq!(uid_item_count(&response, 73), 0);
}

#[test]
fn seen_transition_adds_the_updated_flags_item() {
    let mut stored = message(73);
    stored.flags =
        crate::mail_flags::MessageFlags::from_flags([crate::mail_flags::SystemFlag::Seen]);
    let response = prepare_internal(
        1,
        &stored,
        &[imap::FetchAttribute::Body {
            peek: false,
            section: imap::BodySection::EntireMessage,
            partial: None,
        }],
        imap::Addressing::Sequence,
        &BTreeSet::new(),
        imap::tokenizer::StringMode::Legacy,
        true,
    )
    .expect("BODY FETCH response after durable Seen update");

    assert_eq!(flags_item_count(&response), 1);
    assert!(
        response
            .windows(b"FLAGS (\\Seen)".len())
            .any(|window| { window == b"FLAGS (\\Seen)" })
    );
}

#[test]
fn seen_transition_does_not_duplicate_requested_flags() {
    let mut stored = message(73);
    stored.flags =
        crate::mail_flags::MessageFlags::from_flags([crate::mail_flags::SystemFlag::Seen]);
    let response = prepare_internal(
        1,
        &stored,
        &[
            imap::FetchAttribute::Flags,
            imap::FetchAttribute::Body {
                peek: false,
                section: imap::BodySection::EntireMessage,
                partial: None,
            },
        ],
        imap::Addressing::Sequence,
        &BTreeSet::new(),
        imap::tokenizer::StringMode::Legacy,
        true,
    )
    .expect("BODY FETCH response with requested FLAGS");

    assert_eq!(flags_item_count(&response), 1);
}

/// Проверяет, что decoded Unicode не попадает в legacy quoted-string.
#[test]
fn envelope_uses_literals_before_utf8_accept_and_quotes_after_enable() {
    let mut stored = message(73);
    stored.bytes = b"Subject: =?UTF-8?B?0J/RgNC40LLQtdGC?=\r\nFrom: =?UTF-8?B?0J/RgNC40LLQtdGC?= <alice@test>\r\n\r\nbody".to_vec();
    let attributes = [imap::FetchAttribute::Envelope];
    let legacy = prepare_internal(
        1,
        &stored,
        &attributes,
        imap::Addressing::Sequence,
        &BTreeSet::new(),
        imap::tokenizer::StringMode::Legacy,
        false,
    )
    .expect("legacy ENVELOPE response");
    let enabled = prepare_internal(
        1,
        &stored,
        &attributes,
        imap::Addressing::Sequence,
        &BTreeSet::new(),
        imap::tokenizer::StringMode::Utf8Accept,
        false,
    )
    .expect("UTF8=ACCEPT ENVELOPE response");

    let literal = "{12}\r\nПривет".as_bytes();
    let quoted = "\"Привет\"".as_bytes();
    assert!(legacy.windows(literal.len()).any(|part| part == literal));
    assert!(enabled.windows(quoted.len()).any(|part| part == quoted));
}
