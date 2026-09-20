//! Типизированный базовый профиль IMAP SEARCH.

use super::tokenizer::Token;
use super::{Addressing, Command};
use crate::mime::header_block::HeaderName;

/// Поддерживаемый предикат над согласованным снимком сообщения.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchKey {
    All,
    Seen,
    Unseen,
    Deleted,
    Undeleted,
    Header(HeaderSelector, String),
    Body(String),
    Text(String),
    Larger(u64),
    Smaller(u64),
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
    let mut tokens = tokens.peekable();
    let mut keys = Vec::new();
    while tokens.peek().is_some() {
        let Some(key) = parse_one(&mut tokens) else {
            return Command::Unsupported;
        };
        keys.push(key);
    }
    let key = match keys.len() {
        0 => return Command::Unsupported,
        1 => keys.into_iter().next().expect("one search key"),
        _ => SearchKey::And(keys),
    };
    Command::Search { addressing, key }
}

fn parse_one(tokens: &mut impl Iterator<Item = Token>) -> Option<SearchKey> {
    let Token::Text(value) = tokens.next()? else {
        return None;
    };
    match value.to_ascii_uppercase().as_str() {
        "ALL" => Some(SearchKey::All),
        "SEEN" => Some(SearchKey::Seen),
        "UNSEEN" => Some(SearchKey::Unseen),
        "DELETED" => Some(SearchKey::Deleted),
        "UNDELETED" => Some(SearchKey::Undeleted),
        "SUBJECT" => text(tokens, HeaderName::Subject),
        "FROM" => text(tokens, HeaderName::From),
        "TO" => text(tokens, HeaderName::To),
        "CC" => text(tokens, HeaderName::Cc),
        "BCC" => text(tokens, HeaderName::Bcc),
        "HEADER" => header(tokens),
        "BODY" => argument(tokens).map(SearchKey::Body),
        "TEXT" => argument(tokens).map(SearchKey::Text),
        "LARGER" => number(tokens).map(SearchKey::Larger),
        "SMALLER" => number(tokens).map(SearchKey::Smaller),
        _ => None,
    }
}

fn text(tokens: &mut impl Iterator<Item = Token>, name: HeaderName) -> Option<SearchKey> {
    argument(tokens).map(|value| SearchKey::Header(HeaderSelector::Known(name), value))
}

fn header(tokens: &mut impl Iterator<Item = Token>) -> Option<SearchKey> {
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

fn argument(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}

fn number(tokens: &mut impl Iterator<Item = Token>) -> Option<u64> {
    argument(tokens)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap;

    #[test]
    fn parses_text_header_size_and_implicit_and() {
        let command = imap::parse("a SEARCH FROM alice SUBJECT report LARGER 12\r\n").command;
        let Command::Search { key, .. } = command else {
            panic!("search command");
        };
        assert!(matches!(key, SearchKey::And(keys) if keys.len() == 3));
    }

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
}
