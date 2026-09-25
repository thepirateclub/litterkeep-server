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
        clear_failed_saved_result(session, &result);
        return session
            .push(format!("{tag} NO SEARCH criteria do not match charset\r\n").as_bytes());
    }
    let documents = match search_documents(&messages, &key) {
        Ok(documents) => documents,
        Err(crate::mime::search_text::SearchTextError::InvalidMessage) => {
            std::iter::repeat_with(|| None)
                .take(messages.len())
                .collect()
        }
    };
    let recent = &session.selected_recent;
    let maximum_sequence = usize_as_u64(messages.len());
    let maximum_uid = messages
        .iter()
        .map(|message| message.uid)
        .max()
        .unwrap_or(0);
    let matching_messages = messages
        .iter()
        .enumerate()
        .zip(&documents)
        .filter_map(|((offset, message), document)| {
            let sequence = usize_as_u64(offset.saturating_add(1));
            let position = MatchPosition {
                sequence,
                maximum_sequence,
                maximum_uid,
                recent,
                saved_search_uids: &session.saved_search_uids,
            };
            matches_key(message, &key, document.as_ref(), &position).then_some(SearchMatch {
                number: match addressing {
                    imap::Addressing::Sequence => offset.saturating_add(1) as u64,
                    imap::Addressing::Uid => message.uid,
                },
                uid: message.uid,
            })
        })
        .collect::<Vec<_>>();
    let values = matching_messages
        .iter()
        .map(|matched| matched.number)
        .collect::<Vec<_>>();
    if result.saves_result() {
        session.saved_search_uids = saved_uids(&result, &matching_messages);
    }
    super::imap_protocol::imap_search_response::write(session, addressing, result, &tag, &values)
}

/// Одна находка хранит внешний номер и UID, не смешивая адресные пространства.
struct SearchMatch {
    number: u64,
    uid: u64,
}

/// Вычисляет RFC 5182 SAVE projection для запрошенных ESEARCH return items.
fn saved_uids(
    request: &imap::search_result::ResultRequest,
    matches: &[SearchMatch],
) -> std::collections::BTreeSet<u64> {
    let imap::search_result::ResultRequest::Extended(items) = request else {
        return std::collections::BTreeSet::new();
    };
    let save_all = matches!(
        (
            items.contains(&imap::search_result::ReturnItem::All),
            items.contains(&imap::search_result::ReturnItem::Count),
            items.contains(&imap::search_result::ReturnItem::Min),
            items.contains(&imap::search_result::ReturnItem::Max),
        ),
        (true, _, _, _) | (_, true, _, _) | (false, false, false, false)
    );
    let minimum = matches.iter().map(|matched| matched.number).min();
    let maximum = matches.iter().map(|matched| matched.number).max();
    matches
        .iter()
        .filter(|matched| match save_all {
            true => true,
            false => is_extreme(matched.number, items, minimum, maximum),
        })
        .map(|matched| matched.uid)
        .collect()
}

/// При SAVE MIN/MAX сохраняет только указанный край найденного набора.
fn is_extreme(
    number: u64,
    items: &[imap::search_result::ReturnItem],
    minimum: Option<u64>,
    maximum: Option<u64>,
) -> bool {
    matches!(
        (
            items.contains(&imap::search_result::ReturnItem::Min),
            Some(number) == minimum,
            items.contains(&imap::search_result::ReturnItem::Max),
            Some(number) == maximum,
        ),
        (true, true, _, _) | (_, _, true, true)
    )
}

/// Ошибка SEARCH с SAVE очищает сохранённый результат по RFC 5182.
fn clear_failed_saved_result(session: &mut Session, request: &imap::search_result::ResultRequest) {
    if request.saves_result() {
        session.saved_search_uids.clear();
    }
}

/// Строит MIME-проекции до вычисления совпадений.
fn search_documents(
    messages: &[crate::store::Message],
    key: &SearchKey,
) -> Result<Vec<Option<SearchDocument>>, crate::mime::search_text::SearchTextError> {
    messages
        .iter()
        .map(|message| match requires_headers(key) {
            false => Ok(None),
            true => match SearchDocument::parse(&message.bytes, requires_body(key)) {
                Ok(document) => Ok(Some(document)),
                Err(crate::mime::search_text::SearchTextError::InvalidMessage) => Ok(None),
            },
        })
        .collect()
}

/// Координаты сообщения в одном immutable mailbox snapshot.
struct MatchPosition<'a> {
    sequence: u64,
    maximum_sequence: u64,
    maximum_uid: u64,
    recent: &'a std::collections::BTreeSet<u64>,
    saved_search_uids: &'a std::collections::BTreeSet<u64>,
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
        SearchKey::Keyword(keyword) => message.keywords.contains(keyword),
        SearchKey::Unkeyword(keyword) => !message.keywords.contains(keyword),
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
        SearchKey::Saved => position.saved_search_uids.contains(&message.uid),
        SearchKey::Envelope(field, needle) => {
            document.is_some_and(|value| value.envelope_contains(*field, needle))
        }
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
        | SearchKey::Envelope(..)
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
