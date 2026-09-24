//! Формирование IMAP SEARCH response из согласованного mailbox snapshot.

use super::*;
use crate::imap::search::{HeaderSelector, SearchKey};
use crate::mime::header_block::HeaderName;
use crate::mime::search_text::SearchDocument;

/// Публикует только числа пространства адресации, выбранного клиентом.
pub(super) fn completed(
    session: &mut Session,
    addressing: imap::Addressing,
    key: SearchKey,
    charset: imap::search_charset::Charset,
    result: imap::search_result::ResultRequest,
    tag: String,
    messages: Vec<crate::store::Message>,
) -> io::Result<()> {
    if !charset.accepts(&key) {
        return session
            .push(format!("{tag} NO SEARCH criteria do not match charset\r\n").as_bytes());
    }
    let recent = &session.selected_recent;
    let maximum_sequence = usize_as_u64(messages.len());
    let maximum_uid = messages
        .iter()
        .map(|message| message.uid)
        .max()
        .unwrap_or(0);
    let matches = messages
        .iter()
        .enumerate()
        .filter_map(|(offset, message)| {
            let document = match requires_headers(&key) {
                true => SearchDocument::parse(&message.bytes, requires_body(&key)).ok(),
                false => None,
            };
            let sequence = usize_as_u64(offset.saturating_add(1));
            let position = MatchPosition {
                sequence,
                maximum_sequence,
                maximum_uid,
                recent,
            };
            matches_key(message, &key, document.as_ref(), &position).then_some(match addressing {
                imap::Addressing::Sequence => offset.saturating_add(1) as u64,
                imap::Addressing::Uid => message.uid,
            })
        })
        .collect::<Vec<_>>();
    super::imap_protocol::imap_search_response::write(session, addressing, result, &tag, &matches)
}

/// Координаты сообщения в одном immutable mailbox snapshot.
struct MatchPosition<'a> {
    sequence: u64,
    maximum_sequence: u64,
    maximum_uid: u64,
    recent: &'a std::collections::BTreeSet<u64>,
}

fn matches_key(
    message: &crate::store::Message,
    key: &SearchKey,
    document: Option<&SearchDocument>,
    position: &MatchPosition<'_>,
) -> bool {
    use crate::mail_flags::SystemFlag;
    match key {
        SearchKey::All => true,
        SearchKey::Seen => message.flags.contains(SystemFlag::Seen),
        SearchKey::Unseen => !message.flags.contains(SystemFlag::Seen),
        SearchKey::Answered => message.flags.contains(SystemFlag::Answered),
        SearchKey::Unanswered => !message.flags.contains(SystemFlag::Answered),
        SearchKey::Flagged => message.flags.contains(SystemFlag::Flagged),
        SearchKey::Unflagged => !message.flags.contains(SystemFlag::Flagged),
        SearchKey::Deleted => message.flags.contains(SystemFlag::Deleted),
        SearchKey::Undeleted => !message.flags.contains(SystemFlag::Deleted),
        SearchKey::Draft => message.flags.contains(SystemFlag::Draft),
        SearchKey::Undraft => !message.flags.contains(SystemFlag::Draft),
        SearchKey::Recent => position.recent.contains(&message.uid),
        SearchKey::Old => !position.recent.contains(&message.uid),
        SearchKey::New => match position.recent.contains(&message.uid) {
            true => !message.flags.contains(SystemFlag::Seen),
            false => false,
        },
        SearchKey::SequenceSet(set) => {
            set.contains_bounded(position.sequence, position.maximum_sequence)
        }
        SearchKey::UidSet(set) => set.contains_bounded(message.uid, position.maximum_uid),
        SearchKey::Header(selector, needle) => header_matches(document, selector, needle),
        SearchKey::Body(needle) => document.is_some_and(|value| value.body_contains(needle)),
        SearchKey::Text(needle) => document.is_some_and(|value| value.text_contains(needle)),
        SearchKey::Larger(size) => usize_as_u64(message.bytes.len()) > *size,
        SearchKey::Smaller(size) => usize_as_u64(message.bytes.len()) < *size,
        SearchKey::Before(date) => message_date(message) < *date,
        SearchKey::On(date) => message_date(message) == *date,
        SearchKey::Since(date) => message_date(message) >= *date,
        SearchKey::SentBefore(date) => sent_date(document).is_some_and(|sent| sent < *date),
        SearchKey::SentOn(date) => sent_date(document).is_some_and(|sent| sent == *date),
        SearchKey::SentSince(date) => sent_date(document).is_some_and(|sent| sent >= *date),
        SearchKey::Not(item) => !matches_key(message, item, document, position),
        SearchKey::Or(left, right) => match matches_key(message, left, document, position) {
            true => true,
            false => matches_key(message, right, document, position),
        },
        SearchKey::And(keys) => keys
            .iter()
            .all(|item| matches_key(message, item, document, position)),
    }
}

fn message_date(message: &crate::store::Message) -> crate::imap::search_date::SearchDate {
    crate::imap::search_date::SearchDate::from_mail_date(message.internal_date)
}

fn sent_date(document: Option<&SearchDocument>) -> Option<crate::imap::search_date::SearchDate> {
    let document = document?;
    document
        .headers
        .iter()
        .find(|field| field.kind == HeaderName::Date)
        .and_then(|field| crate::imap::sent_date::parse(field.value.as_bytes()))
}

fn header_matches(
    document: Option<&SearchDocument>,
    selector: &HeaderSelector,
    needle: &str,
) -> bool {
    let Some(document) = document else {
        return false;
    };
    document.headers.iter().any(|field| {
        selector_matches(field.kind, &field.name, selector)
            && crate::mime::search_text::contains_case_insensitive_ascii(&field.value, needle)
    })
}

fn selector_matches(kind: HeaderName, name: &str, selector: &HeaderSelector) -> bool {
    match selector {
        HeaderSelector::Known(expected) => kind == *expected,
        HeaderSelector::Other(expected) => name == expected,
    }
}

fn requires_headers(key: &SearchKey) -> bool {
    match key {
        SearchKey::Header(..)
        | SearchKey::Body(_)
        | SearchKey::Text(_)
        | SearchKey::SentBefore(_)
        | SearchKey::SentOn(_)
        | SearchKey::SentSince(_) => true,
        SearchKey::And(keys) => keys.iter().any(requires_headers),
        SearchKey::Not(item) => requires_headers(item),
        SearchKey::Or(left, right) => match requires_headers(left) {
            true => true,
            false => requires_headers(right),
        },
        _ => false,
    }
}

fn requires_body(key: &SearchKey) -> bool {
    match key {
        SearchKey::Body(_) | SearchKey::Text(_) => true,
        SearchKey::And(keys) => keys.iter().any(requires_body),
        SearchKey::Not(item) => requires_body(item),
        SearchKey::Or(left, right) => requires_body(left) || requires_body(right),
        _ => false,
    }
}

fn usize_as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "imap_search_tests.rs"]
mod tests;
