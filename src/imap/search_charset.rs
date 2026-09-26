//! Типизация и проверка кодировки строковых критериев IMAP SEARCH.

use super::search::{HeaderSelector, SearchKey};
use super::tokenizer::Token;
use std::collections::VecDeque;

/// Кодировка строк поиска, выбранная клиентом или профилем протокола.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Charset {
    UsAscii,
    Utf8,
    Unsupported,
}

impl Charset {
    /// Нормализует имя charset в закрытый набор поддерживаемых значений.
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "US-ASCII" => Self::UsAscii,
            "UTF-8" => Self::Utf8,
            _ => Self::Unsupported,
        }
    }

    /// Проверяет, что критерии представимы в явно объявленной кодировке.
    pub fn accepts(self, key: &SearchKey) -> bool {
        match self {
            Self::UsAscii => key_is_ascii(key),
            Self::Utf8 => true,
            Self::Unsupported => false,
        }
    }
}

/// Разбирает необязательный charset prefix перед SEARCH search-program.
pub(super) fn parse_prefix(tokens: &mut VecDeque<Token>) -> Option<Charset> {
    let requested = match tokens.front() {
        Some(Token::Text(value)) => value.eq_ignore_ascii_case("CHARSET"),
        _ => false,
    };
    match requested {
        false => Some(Charset::Utf8),
        true => {
            tokens.pop_front();
            match tokens.pop_front()? {
                Token::Text(value) => Some(Charset::parse(&value)),
                Token::Quoted(_) | Token::Literal(_) | Token::ListStart | Token::ListEnd => None,
            }
        }
    }
}

fn key_is_ascii(key: &SearchKey) -> bool {
    match key {
        SearchKey::Envelope(_, value) => value.is_ascii(),
        SearchKey::Keyword(_) | SearchKey::Unkeyword(_) => true,
        SearchKey::Header(selector, value) => selector_is_ascii(selector) && value.is_ascii(),
        SearchKey::Body(value) | SearchKey::Text(value) => value.is_ascii(),
        SearchKey::Not(item) => key_is_ascii(item),
        SearchKey::Or(left, right) => key_is_ascii(left) && key_is_ascii(right),
        SearchKey::And(items) => items.iter().all(key_is_ascii),
        _ => true,
    }
}

fn selector_is_ascii(selector: &HeaderSelector) -> bool {
    match selector {
        HeaderSelector::Known(_) => true,
        HeaderSelector::Other(name) => name.is_ascii(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Явный US-ASCII допускает только ASCII-строки даже внутри OR/NOT.
    #[test]
    fn ascii_charset_checks_nested_text_criteria() {
        let ascii = SearchKey::Header(
            HeaderSelector::Known(crate::mime::header_block::HeaderName::Subject),
            "invoice".into(),
        );
        let unicode = SearchKey::Body("café".into());
        let nested = SearchKey::Not(Box::new(SearchKey::Or(
            Box::new(ascii.clone()),
            Box::new(unicode),
        )));
        assert!(Charset::UsAscii.accepts(&ascii));
        assert!(!Charset::UsAscii.accepts(&nested));
        assert!(Charset::Utf8.accepts(&nested));
    }
}
