//! Типизированные атрибуты и grammar команды FETCH.

use super::sequence_set::SequenceSet;
use super::tokenizer::Token;
use super::{Addressing, Command};

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
    let sequence_set = next_text(tokens).and_then(|word| SequenceSet::parse(&word).ok());
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
                Token::Text(value) => attributes.extend(parse_request_item(&value)?),
                Token::ListStart => return None,
            }
        },
        Token::Text(value) => attributes.extend(parse_request_item(&value)?),
        Token::ListEnd => return None,
    }
    match attributes.is_empty() {
        true => None,
        false => Some(attributes),
    }
}

fn parse_request_item(value: &str) -> Option<Vec<FetchAttribute>> {
    match FetchMacro::parse(value) {
        Some(fetch_macro) => Some(fetch_macro.attributes()),
        None => Some(vec![parse_attribute(value)?]),
    }
}

fn parse_attribute(value: &str) -> Option<FetchAttribute> {
    let token = value.trim_matches(['(', ')']);
    match token.to_ascii_uppercase().as_str() {
        "RFC822" => Some(FetchAttribute::Rfc822),
        "RFC822.SIZE" => Some(FetchAttribute::Rfc822Size),
        "BODYSTRUCTURE" => Some(FetchAttribute::BodyStructure),
        "ENVELOPE" => Some(FetchAttribute::Envelope),
        "FLAGS" => Some(FetchAttribute::Flags),
        "INTERNALDATE" => Some(FetchAttribute::InternalDate),
        "UID" => Some(FetchAttribute::Uid),
        _ => parse_body_attribute(token),
    }
}

fn parse_body_attribute(token: &str) -> Option<FetchAttribute> {
    let upper = token.to_ascii_uppercase();
    let (peek, after_prefix) = match (
        upper.strip_prefix("BODY.PEEK["),
        upper.strip_prefix("BODY["),
    ) {
        (Some(value), _) => (true, value),
        (None, Some(value)) => (false, value),
        (None, None) => return None,
    };
    let (section_text, suffix) = after_prefix.split_once(']')?;
    let section = parse_body_section(section_text)?;
    let partial = match suffix {
        "" => None,
        value => Some(parse_partial(value)?),
    };
    Some(FetchAttribute::Body {
        peek,
        section,
        partial,
    })
}

fn parse_body_section(value: &str) -> Option<BodySection> {
    if value.is_empty() {
        return Some(BodySection::EntireMessage);
    }
    let mut path = Vec::new();
    let mut mime_headers = false;
    for component in value.split('.') {
        match component {
            "MIME" => mime_headers = true,
            number => path.push(number.parse::<usize>().ok()?),
        }
    }
    match path.is_empty() {
        true => None,
        false => Some(BodySection::Part { path, mime_headers }),
    }
}

fn parse_partial(value: &str) -> Option<Partial> {
    let inner = value.strip_prefix('<')?.strip_suffix('>')?;
    let (offset, count) = inner.split_once('.')?;
    Some(Partial {
        offset: offset.parse().ok()?,
        count: count.parse().ok()?,
    })
}

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    tokens.find_map(|token| match token {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    })
}
