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
    let matches = messages
        .iter()
        .enumerate()
        .filter_map(|(offset, message)| {
            let headers = match requires_headers(&key) {
                true => crate::mime::header_block::parse(&message.bytes).ok(),
                false => None,
            };
            matches_key(message, &key, headers.as_ref()).then_some(match addressing {
                imap::Addressing::Sequence => offset.saturating_add(1) as u64,
                imap::Addressing::Uid => message.uid,
            })
        })
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    session.push(format!("* SEARCH {}\r\n", matches.join(" ")).as_bytes())?;
    session.push(format!("{tag} OK SEARCH completed\r\n").as_bytes())
}

fn matches_key(
    message: &crate::store::Message,
    key: &SearchKey,
    headers: Option<&HeaderBlock>,
) -> bool {
    use crate::mail_flags::SystemFlag;
    match key {
        SearchKey::All => true,
        SearchKey::Seen => message.flags.contains(SystemFlag::Seen),
        SearchKey::Unseen => !message.flags.contains(SystemFlag::Seen),
        SearchKey::Deleted => message.flags.contains(SystemFlag::Deleted),
        SearchKey::Undeleted => !message.flags.contains(SystemFlag::Deleted),
        SearchKey::Header(selector, needle) => header_matches(headers, selector, needle),
        SearchKey::Body(needle) => body_matches(message, headers, needle),
        SearchKey::Text(needle) => contains(&message.bytes, needle.as_bytes()),
        SearchKey::Larger(size) => usize_as_u64(message.bytes.len()) > *size,
        SearchKey::Smaller(size) => usize_as_u64(message.bytes.len()) < *size,
        SearchKey::Before(date) => message_date(message) < *date,
        SearchKey::On(date) => message_date(message) == *date,
        SearchKey::Since(date) => message_date(message) >= *date,
        SearchKey::Not(item) => !matches_key(message, item, headers),
        SearchKey::Or(left, right) => match matches_key(message, left, headers) {
            true => true,
            false => matches_key(message, right, headers),
        },
        SearchKey::And(keys) => keys.iter().all(|item| matches_key(message, item, headers)),
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
mod tests {
    use super::*;
    use crate::mail_date::MailDate;
    use crate::mail_flags::MessageFlags;
    use std::path::PathBuf;

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
        let headers = crate::mime::header_block::parse(&value.bytes).expect("headers");
        assert!(matches_key(&value, &query, Some(&headers)));
        assert!(!matches_key(&value, &SearchKey::Smaller(20), None));
        assert!(matches_key(
            &value,
            &SearchKey::Text("subject".into()),
            None
        ));
    }

    #[test]
    fn matches_internal_date_and_boolean_tree() {
        let mut value = message(b"Subject: report\r\n\r\nbody");
        value.internal_date = MailDate::parse("2-Feb-2026 12:00:00 +0000").expect("date");
        let boundary =
            crate::imap::search_date::SearchDate::parse("2-Feb-2026").expect("search date");
        let query = SearchKey::Or(
            Box::new(SearchKey::Before(boundary)),
            Box::new(SearchKey::Not(Box::new(SearchKey::Seen))),
        );
        assert!(matches_key(&value, &query, None));
        assert!(matches_key(&value, &SearchKey::On(boundary), None));
        assert!(matches_key(&value, &SearchKey::Since(boundary), None));
        assert!(!matches_key(&value, &SearchKey::Before(boundary), None));
    }
}
