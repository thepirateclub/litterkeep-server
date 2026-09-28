//! Проверки двухфазной семантики Seen без сетевого окружения.

use super::*;
use crate::mail_date::MailDate;
use crate::store::{MailboxSnapshot, Message};
use std::path::PathBuf;

fn snapshot() -> MailboxSnapshot {
    MailboxSnapshot {
        uid_validity: 1,
        uid_next: 3,
        messages: vec![Message {
            uid: 1,
            path: PathBuf::from("message-1.eml"),
            bytes: b"Subject: unread\r\n\r\nbody".to_vec(),
            flags: crate::mail_flags::MessageFlags::default(),
            keywords: std::collections::BTreeSet::new(),
            internal_date: MailDate::now(),
        }],
    }
}

fn pending(peek: bool, read_only: bool) -> Pending {
    Pending::ReadForFetch {
        addressing: imap::Addressing::Sequence,
        sequence_set: imap::sequence_set::MessageSet::Explicit(
            imap::sequence_set::SequenceSet::parse("1").unwrap(),
        ),
        attributes: vec![imap::FetchAttribute::Body {
            peek,
            section: imap::BodySection::EntireMessage,
            partial: None,
        }],
        partial: None,
        tag: "A1".to_owned(),
        mailbox: "alice/INBOX".to_owned(),
        read_only,
        saved_uids: Default::default(),
    }
}

#[test]
fn ordinary_body_queues_durable_seen_for_selected_unread_uid() {
    let response = Response::Read {
        request_id: 1,
        result: Ok(snapshot()),
    };
    let mut next_id = 2;
    let (request, follow_up) = follow_up(&pending(false, false), &response, &mut next_id).unwrap();
    assert!(matches!(
        request,
        Request::UpdateFlags {
            request_id: 2,
            uids,
            mutation: crate::mail_flags::FlagMutation::Add,
            flags,
            ..
        } if uids == vec![1]
            && flags.contains(crate::mail_flags::SystemFlag::Seen)
    ));
    assert!(matches!(follow_up, Pending::FetchReadForSeen { uids, .. } if uids == vec![1]));
    assert_eq!(next_id, 3);
}

#[test]
fn peek_and_read_only_body_do_not_mutate_seen() {
    let response = Response::Read {
        request_id: 1,
        result: Ok(snapshot()),
    };
    let mut next_id = 2;
    assert!(follow_up(&pending(true, false), &response, &mut next_id).is_none());
    assert!(follow_up(&pending(false, true), &response, &mut next_id).is_none());
    assert_eq!(next_id, 2);
}

/// Ошибка любого FETCH item не должна оставлять durable Seen побочным эффектом.
#[test]
fn failed_combined_fetch_does_not_queue_seen_mutation() {
    let mut message = snapshot();
    if let Some(stored) = message.messages.first_mut() {
        stored.bytes =
            b"Content-Type: text/plain\r\nContent-Transfer-Encoding: x-unknown\r\n\r\nbody"
                .to_vec();
    }
    let response = Response::Read {
        request_id: 1,
        result: Ok(message),
    };
    let mut request = pending(false, false);
    if let Pending::ReadForFetch { attributes, .. } = &mut request {
        attributes.push(imap::FetchAttribute::Binary {
            section: vec![1],
            peek: false,
            partial: None,
        });
    }
    let mut next_id = 2;

    assert!(follow_up(&request, &response, &mut next_id).is_none());
    assert_eq!(next_id, 2);
}
