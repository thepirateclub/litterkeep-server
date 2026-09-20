//! Формирование IMAP SEARCH response из согласованного mailbox snapshot.

use super::*;
use crate::imap::search::{HeaderSelector, SearchKey};
use crate::mime::header_block::{HeaderBlock, HeaderName};

/// Публикует только числа пространства адресации, выбранного клиентом.
pub(super) fn completed(
    session: &mut Session,
    addressing: imap::Addressing,
    key: SearchKey,
    tag: String,
    messages: Vec<crate::store::Message>,
) -> io::Result<()> {
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
            let headers = match requires_headers(&key) {
                true => crate::mime::header_block::parse(&message.bytes).ok(),
                false => None,
            };
            let sequence = usize_as_u64(offset.saturating_add(1));
            let position = MatchPosition {
                sequence,
                maximum_sequence,
                maximum_uid,
                recent,
            };
            matches_key(message, &key, headers.as_ref(), &position).then_some(match addressing {
                imap::Addressing::Sequence => offset.saturating_add(1) as u64,
                imap::Addressing::Uid => message.uid,
            })
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    session.push(format!("* SEARCH {}\r\n", matches.join(" ")).as_bytes())?;
    session.push(format!("{tag} OK SEARCH completed\r\n").as_bytes())
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
    headers: Option<&HeaderBlock>,
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
        SearchKey::Header(selector, needle) => header_matches(headers, selector, needle),
        SearchKey::Body(needle) => body_matches(message, headers, needle),
        SearchKey::Text(needle) => contains(&message.bytes, needle.as_bytes()),
        SearchKey::Larger(size) => usize_as_u64(message.bytes.len()) > *size,
        SearchKey::Smaller(size) => usize_as_u64(message.bytes.len()) < *size,
        SearchKey::Before(date) => message_date(message) < *date,
        SearchKey::On(date) => message_date(message) == *date,
        SearchKey::Since(date) => message_date(message) >= *date,
        SearchKey::Not(item) => !matches_key(message, item, headers, position),
        SearchKey::Or(left, right) => match matches_key(message, left, headers, position) {
            true => true,
            false => matches_key(message, right, headers, position),
        },
        SearchKey::And(keys) => keys
            .iter()
            .all(|item| matches_key(message, item, headers, position)),
    }
}

fn message_date(message: &crate::store::Message) -> crate::imap::search_date::SearchDate {
    crate::imap::search_date::SearchDate::from_mail_date(message.internal_date)
}

fn header_matches(headers: Option<&HeaderBlock>, selector: &HeaderSelector, needle: &str) -> bool {
    let Some(headers) = headers else {
        return false;
    };
    headers.fields.iter().any(|field| {
        selector_matches(field.kind(), &field.name, selector)
            && contains(&field.value, needle.as_bytes())
    })
}

fn selector_matches(kind: HeaderName, name: &str, selector: &HeaderSelector) -> bool {
    match selector {
        HeaderSelector::Known(expected) => kind == *expected,
        HeaderSelector::Other(expected) => name == expected,
    }
}

fn body_matches(
    message: &crate::store::Message,
    headers: Option<&HeaderBlock>,
    needle: &str,
) -> bool {
    let Some(HeaderBlock { body_offset, .. }) = headers else {
        return false;
    };
    message
        .bytes
        .get(*body_offset..)
        .is_some_and(|body| contains(body, needle.as_bytes()))
}

fn requires_headers(key: &SearchKey) -> bool {
    match key {
        SearchKey::Header(..) | SearchKey::Body(_) => true,
        SearchKey::And(keys) => keys.iter().any(requires_headers),
        SearchKey::Not(item) => requires_headers(item),
        SearchKey::Or(left, right) => match requires_headers(left) {
            true => true,
            false => requires_headers(right),
        },
        _ => false,
    }
}

/// RFC SEARCH string matching является регистронезависимым для ASCII профиля.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    match needle.is_empty() {
        true => true,
        false => haystack
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle)),
    }
}

fn usize_as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "imap_search_tests.rs"]
mod tests;
