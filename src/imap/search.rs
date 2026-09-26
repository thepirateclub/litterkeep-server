//! Типизированный базовый профиль IMAP SEARCH.

use super::tokenizer::Token;
use super::{Addressing, Command};
use crate::mail_flags::KeywordFlag;
use crate::mime::header_block::HeaderName;
use std::collections::VecDeque;

/// Поддерживаемый предикат над согласованным снимком сообщения.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchKey {
    All,
    Seen,
    Unseen,
    Answered,
    Unanswered,
    Flagged,
    Unflagged,
    Deleted,
    Undeleted,
    Draft,
    Undraft,
    Keyword(KeywordFlag),
    Unkeyword(KeywordFlag),
    Recent,
    Old,
    New,
    SequenceSet(super::sequence_set::SequenceSet),
    UidSet(super::sequence_set::SequenceSet),
    /// Сообщение присутствует в сохранённом SEARCH result этой сессии.
    Saved,
    /// Поля сообщения, сопоставляемые с IMAP ENVELOPE, а не сырым заголовком.
    Envelope(HeaderName, String),
    Header(HeaderSelector, String),
    Body(String),
    Text(String),
    Larger(u64),
    Smaller(u64),
    Before(super::search_date::SearchDate),
    On(super::search_date::SearchDate),
    Since(super::search_date::SearchDate),
    SentBefore(super::search_date::SearchDate),
    SentOn(super::search_date::SearchDate),
    SentSince(super::search_date::SearchDate),
    Not(Box<SearchKey>),
    Or(Box<SearchKey>, Box<SearchKey>),
    /// Последовательные criteria в SEARCH имеют семантику логического AND.
    And(Vec<SearchKey>),
}

/// Имя заголовка хранится enum-ом, кроме расширяемого RFC field-name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderSelector {
    Known(HeaderName),
    Other(String),
}

/// Разбирает поддерживаемую последовательность criteria без ложного успеха.
pub fn parse(tokens: impl Iterator<Item = Token>, addressing: Addressing) -> Command {
    let mut tokens = tokens.collect::<VecDeque<_>>();
    let Some(result) = super::search_result::parse_prefix(&mut tokens) else {
        return Command::Unsupported;
    };
    let charset_requested =
        matches!(tokens.front(), Some(Token::Text(value)) if value.eq_ignore_ascii_case("CHARSET"));
    let Some(charset) = super::search_charset::parse_prefix(&mut tokens) else {
        return Command::Unsupported;
    };
    let Some(key) = parse_group(&mut tokens, false) else {
        return Command::Unsupported;
    };
    match tokens.is_empty() {
        true => Command::Search {
            addressing,
            key,
            charset,
            charset_explicit: charset_requested,
            result,
        },
        false => Command::Unsupported,
    }
}

fn parse_group(tokens: &mut VecDeque<Token>, nested: bool) -> Option<SearchKey> {
    let mut keys = Vec::new();
    loop {
        match tokens.front() {
            Some(Token::ListEnd) if nested => {
                tokens.pop_front();
                break;
            }
            Some(Token::ListEnd) => return None,
            Some(_) => keys.push(parse_one(tokens)?),
            None if nested => return None,
            None => break,
        }
    }
    match keys.len() {
        0 => None,
        1 => Some(keys.into_iter().next().expect("one search key")),
        _ => Some(SearchKey::And(keys)),
    }
}

fn parse_one(tokens: &mut VecDeque<Token>) -> Option<SearchKey> {
    let value = match tokens.pop_front()? {
        Token::Text(value) => value,
        Token::Quoted(_) => return None,
        Token::ListStart => return parse_group(tokens, true),
        Token::ListEnd => return None,
    };
    match value.to_ascii_uppercase().as_str() {
        "ALL" => Some(SearchKey::All),
        "SEEN" => Some(SearchKey::Seen),
        "UNSEEN" => Some(SearchKey::Unseen),
        "ANSWERED" => Some(SearchKey::Answered),
        "UNANSWERED" => Some(SearchKey::Unanswered),
        "FLAGGED" => Some(SearchKey::Flagged),
        "UNFLAGGED" => Some(SearchKey::Unflagged),
        "DELETED" => Some(SearchKey::Deleted),
        "UNDELETED" => Some(SearchKey::Undeleted),
        "DRAFT" => Some(SearchKey::Draft),
        "UNDRAFT" => Some(SearchKey::Undraft),
        "RECENT" => Some(SearchKey::Recent),
        "OLD" => Some(SearchKey::Old),
        "NEW" => Some(SearchKey::New),
        "UID" => sequence_set(tokens).map(SearchKey::UidSet),
        "KEYWORD" => keyword(tokens).map(SearchKey::Keyword),
        "UNKEYWORD" => keyword(tokens).map(SearchKey::Unkeyword),
        "SUBJECT" => envelope(tokens, HeaderName::Subject),
        "FROM" => envelope(tokens, HeaderName::From),
        "TO" => envelope(tokens, HeaderName::To),
        "CC" => envelope(tokens, HeaderName::Cc),
        "BCC" => envelope(tokens, HeaderName::Bcc),
        "HEADER" => header(tokens),
        "BODY" => argument(tokens).map(SearchKey::Body),
        "TEXT" => argument(tokens).map(SearchKey::Text),
        "LARGER" => number(tokens).map(SearchKey::Larger),
        "SMALLER" => number(tokens).map(SearchKey::Smaller),
        "BEFORE" => date(tokens).map(SearchKey::Before),
        "ON" => date(tokens).map(SearchKey::On),
        "SINCE" => date(tokens).map(SearchKey::Since),
        "SENTBEFORE" => date(tokens).map(SearchKey::SentBefore),
        "SENTON" => date(tokens).map(SearchKey::SentOn),
        "SENTSINCE" => date(tokens).map(SearchKey::SentSince),
        "NOT" => parse_one(tokens).map(Box::new).map(SearchKey::Not),
        "OR" => {
            let left = parse_one(tokens)?;
            let right = parse_one(tokens)?;
            Some(SearchKey::Or(Box::new(left), Box::new(right)))
        }
        "$" => Some(SearchKey::Saved),
        _ => super::sequence_set::SequenceSet::parse(&value)
            .ok()
            .map(SearchKey::SequenceSet),
    }
}

fn keyword(tokens: &mut VecDeque<Token>) -> Option<KeywordFlag> {
    KeywordFlag::parse(&argument(tokens)?)
}

fn envelope(tokens: &mut VecDeque<Token>, name: HeaderName) -> Option<SearchKey> {
    argument(tokens).map(|value| SearchKey::Envelope(name, value))
}

fn header(tokens: &mut VecDeque<Token>) -> Option<SearchKey> {
    let name = argument(tokens)?;
    let value = argument(tokens)?;
    let selector = match known_header(&name) {
        Some(known) => HeaderSelector::Known(known),
        None => HeaderSelector::Other(name.to_ascii_lowercase()),
    };
    Some(SearchKey::Header(selector, value))
}

fn known_header(value: &str) -> Option<HeaderName> {
    match value.to_ascii_uppercase().as_str() {
        "SUBJECT" => Some(HeaderName::Subject),
        "FROM" => Some(HeaderName::From),
        "TO" => Some(HeaderName::To),
        "CC" => Some(HeaderName::Cc),
        "BCC" => Some(HeaderName::Bcc),
        "DATE" => Some(HeaderName::Date),
        "MESSAGE-ID" => Some(HeaderName::MessageId),
        _ => None,
    }
}

fn argument(tokens: &mut VecDeque<Token>) -> Option<String> {
    match tokens.pop_front()? {
        Token::Text(value) => Some(value),
        Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}

fn number(tokens: &mut VecDeque<Token>) -> Option<u64> {
    argument(tokens)?.parse().ok()
}

fn date(tokens: &mut VecDeque<Token>) -> Option<super::search_date::SearchDate> {
    super::search_date::SearchDate::parse(&argument(tokens)?)
}

fn sequence_set(tokens: &mut VecDeque<Token>) -> Option<super::sequence_set::SequenceSet> {
    super::sequence_set::SequenceSet::parse(&argument(tokens)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap;

    #[test]
    fn rejects_missing_argument_and_unknown_criterion() {
        assert_eq!(
            imap::parse("a SEARCH SUBJECT\r\n").command,
            Command::Unsupported
        );
        assert_eq!(
            imap::parse("a SEARCH MAGIC value\r\n").command,
            Command::Unsupported
        );
    }

    #[test]
    fn parses_dates_boolean_operators_and_groups() {
        let command = imap::parse(
            "a SEARCH SINCE 1-Feb-2026 SENTBEFORE 3-Feb-2026 SENTON 2-Feb-2026 \
             SENTSINCE 1-Feb-2026 OR SUBJECT one (NOT SEEN SMALLER 99)\r\n",
        )
        .command;
        assert!(matches!(
            command,
            Command::Search {
                key: SearchKey::And(_),
                ..
            }
        ));
        assert_eq!(
            imap::parse("a SEARCH (SEEN\r\n").command,
            Command::Unsupported
        );
    }

    #[test]
    fn parses_system_and_session_flag_criteria() {
        let command = imap::parse(
            "a SEARCH ANSWERED UNANSWERED FLAGGED UNFLAGGED DRAFT UNDRAFT RECENT OLD NEW\r\n",
        )
        .command;
        assert!(matches!(
            command,
            Command::Search {
                key: SearchKey::And(keys),
                ..
            } if keys.len() == 9
        ));
    }

    #[test]
    fn parses_sequence_and_uid_set_criteria() {
        let command = imap::parse("a UID SEARCH 2:4 UID 90:*\r\n").command;
        assert!(matches!(
            command,
            Command::Search {
                addressing: Addressing::Uid,
                key: SearchKey::And(keys),
                ..
            } if keys.len() == 2
        ));
        assert_eq!(
            imap::parse("a SEARCH UID unknown\r\n").command,
            Command::Unsupported
        );
    }

    #[test]
    fn parses_saved_search_reference_as_a_typed_key() {
        let command = imap::parse("a SEARCH (OR $ 3:5)\r\n").command;
        assert!(matches!(
            command,
            Command::Search {
                key: SearchKey::Or(left, _),
                ..
            } if matches!(*left, SearchKey::Saved)
        ));
        assert!(matches!(
            imap::parse("a FETCH $ FLAGS\r\n").command,
            Command::Fetch {
                sequence_set: super::super::sequence_set::MessageSet::Saved,
                ..
            }
        ));
    }
}
