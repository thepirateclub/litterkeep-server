//! Типизированные атрибуты и grammar команды FETCH.

use super::sequence_set::MessageSet;
use super::tokenizer::Token;
use super::{Addressing, Command, fetch_section};

/// Один элемент стандартного набора данных FETCH.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchAttribute {
    Rfc822,
    Rfc822Size,
    BodyStructure,
    Envelope,
    Flags,
    InternalDate,
    Uid,
    /// Декодированные октеты MIME leaf part и правила Seen для PEEK.
    Binary {
        section: Vec<usize>,
        peek: bool,
        partial: Option<Partial>,
    },
    /// Размер декодированного MIME leaf part без изменения флагов сообщения.
    BinarySize {
        section: Vec<usize>,
    },
    Body {
        peek: bool,
        section: BodySection,
        partial: Option<Partial>,
    },
}

/// Выбираемая raw MIME section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodySection {
    EntireMessage,
    Headers {
        path: Vec<usize>,
    },
    Text {
        path: Vec<usize>,
    },
    HeaderFields {
        path: Vec<usize>,
        names: Vec<String>,
        excluded: bool,
    },
    Part {
        path: Vec<usize>,
        mime_headers: bool,
    },
}

/// Октетный диапазон IMAP partial fetch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Partial {
    pub offset: usize,
    pub count: usize,
}

/// Сокращённые наборы данных, определённые базовым IMAP FETCH.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FetchMacro {
    All,
    Fast,
    Full,
}

impl FetchMacro {
    /// Преобразует wire atom в закрытый набор поддерживаемых макросов.
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_uppercase().as_str() {
            "ALL" => Some(Self::All),
            "FAST" => Some(Self::Fast),
            "FULL" => Some(Self::Full),
            _ => None,
        }
    }

    /// Раскрывает макрос до тех же типизированных элементов, что и явный список.
    fn attributes(self) -> Vec<FetchAttribute> {
        match self {
            Self::All => vec![
                FetchAttribute::Flags,
                FetchAttribute::InternalDate,
                FetchAttribute::Rfc822Size,
                FetchAttribute::Envelope,
            ],
            Self::Fast => vec![
                FetchAttribute::Flags,
                FetchAttribute::InternalDate,
                FetchAttribute::Rfc822Size,
            ],
            Self::Full => vec![
                FetchAttribute::Flags,
                FetchAttribute::InternalDate,
                FetchAttribute::Rfc822Size,
                FetchAttribute::Envelope,
                FetchAttribute::BodyStructure,
            ],
        }
    }
}

pub(super) fn parse(tokens: &mut impl Iterator<Item = Token>, addressing: Addressing) -> Command {
    let sequence_set = next_text(tokens).and_then(|word| MessageSet::parse(&word).ok());
    let attributes = parse_attribute_list(tokens);
    match (sequence_set, attributes, tokens.next()) {
        (Some(sequence_set), Some(attributes), None) => Command::Fetch {
            addressing,
            sequence_set,
            attributes,
        },
        _ => Command::Unsupported,
    }
}

fn parse_attribute_list(tokens: &mut impl Iterator<Item = Token>) -> Option<Vec<FetchAttribute>> {
    let mut attributes = Vec::new();
    match tokens.next()? {
        Token::ListStart => loop {
            match tokens.next()? {
                Token::ListEnd => break,
                Token::Text(value) => attributes.extend(parse_request_item(&value, tokens)?),
                Token::Quoted(value) => attributes.extend(parse_request_item(&value, tokens)?),
                Token::ListStart => return None,
            }
        },
        Token::Text(value) => attributes.extend(parse_request_item(&value, tokens)?),
        Token::Quoted(value) => attributes.extend(parse_request_item(&value, tokens)?),
        Token::ListEnd => return None,
    }
    match attributes.is_empty() {
        true => None,
        false => Some(attributes),
    }
}

fn parse_request_item(
    value: &str,
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<Vec<FetchAttribute>> {
    match FetchMacro::parse(value) {
        Some(fetch_macro) => Some(fetch_macro.attributes()),
        None => Some(vec![fetch_section::parse(value, tokens)?]),
    }
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    tokens.find_map(|token| match token {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    })
}
