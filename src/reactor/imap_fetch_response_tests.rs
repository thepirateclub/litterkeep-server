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
