//! Разбор BODY section data items и полей вложенных RFC822 headers.

use super::fetch::{BodySection, FetchAttribute, Partial};
use super::tokenizer::Token;

/// Преобразует один FETCH atom, при необходимости забирая поле-name list.
pub(super) fn parse(
    value: &str,
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<FetchAttribute> {
    let token = value.trim_matches(['(', ')']);
    match token.to_ascii_uppercase().as_str() {
        "RFC822" => Some(FetchAttribute::Rfc822),
        "RFC822.SIZE" => Some(FetchAttribute::Rfc822Size),
        "BODYSTRUCTURE" => Some(FetchAttribute::BodyStructure),
        "ENVELOPE" => Some(FetchAttribute::Envelope),
        "FLAGS" => Some(FetchAttribute::Flags),
        "INTERNALDATE" => Some(FetchAttribute::InternalDate),
        "UID" => Some(FetchAttribute::Uid),
        _ => match parse_body_attribute(token) {
            Some(attribute) => Some(attribute),
            None => parse_header_fields_attribute(token, tokens),
        },
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
    match value {
        "HEADER" => Some(BodySection::Headers { path: Vec::new() }),
        "TEXT" => Some(BodySection::Text { path: Vec::new() }),
        "MIME" => None,
        _ => match value.strip_suffix(".HEADER") {
            Some(path) => Some(BodySection::Headers {
                path: parse_numeric_path(path)?,
            }),
            None => match value.strip_suffix(".TEXT") {
                Some(path) => Some(BodySection::Text {
                    path: parse_numeric_path(path)?,
                }),
                None => match value.strip_suffix(".MIME") {
                    Some(path) => Some(BodySection::Part {
                        path: parse_numeric_path(path)?,
                        mime_headers: true,
                    }),
                    None => match parse_numeric_path(value)? {
                        path if path.is_empty() => None,
                        path => Some(BodySection::Part {
                            path,
                            mime_headers: false,
                        }),
                    },
                },
            },
        },
    }
}

fn parse_header_fields_attribute(
    value: &str,
    tokens: &mut impl Iterator<Item = Token>,
) -> Option<FetchAttribute> {
    let upper = value.to_ascii_uppercase();
    let (peek, selector) = match (
        upper.strip_prefix("BODY.PEEK["),
        upper.strip_prefix("BODY["),
    ) {
        (Some(selector), _) => (true, selector),
        (None, Some(selector)) => (false, selector),
        (None, None) => return None,
    };
    let (path_text, excluded) = match selector.strip_suffix(".HEADER.FIELDS.NOT") {
        Some(path) => (path, true),
        None => match selector.strip_suffix("HEADER.FIELDS.NOT") {
            Some("") => ("", true),
            Some(path) => (path.strip_suffix('.')?, true),
            None => match selector.strip_suffix(".HEADER.FIELDS") {
                Some(path) => (path, false),
                None => match selector.strip_suffix("HEADER.FIELDS") {
                    Some("") => ("", false),
                    Some(path) => (path.strip_suffix('.')?, false),
                    None => return None,
                },
            },
        },
    };
    let path = parse_numeric_path(path_text)?;
    let names = parse_field_names(tokens)?;
    let partial = parse_closing_section(tokens)?;
    Some(FetchAttribute::Body {
        peek,
        section: BodySection::HeaderFields {
            path,
            names,
            excluded,
        },
        partial,
    })
}

fn parse_field_names(tokens: &mut impl Iterator<Item = Token>) -> Option<Vec<String>> {
    let mut names = Vec::new();
    match tokens.next()? {
        Token::ListStart => loop {
            match tokens.next()? {
                Token::ListEnd => break,
                Token::Text(name) if valid_field_name(&name) => names.push(name),
                Token::Text(_) | Token::ListStart => return None,
            }
        },
        Token::Text(_) | Token::ListEnd => return None,
    }
    match names.is_empty() {
        true => None,
        false => Some(names),
    }
}

fn valid_field_name(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| matches!(byte, 33..=57 | 59..=126))
}

fn parse_closing_section(tokens: &mut impl Iterator<Item = Token>) -> Option<Option<Partial>> {
    let Token::Text(closing) = tokens.next()? else {
        return None;
    };
    let suffix = closing.strip_prefix(']')?;
    match suffix.is_empty() {
        true => Some(None),
        false => Some(Some(parse_partial(suffix)?)),
    }
}

fn parse_numeric_path(value: &str) -> Option<Vec<usize>> {
    match value.is_empty() {
        true => Some(Vec::new()),
        false => value
            .split('.')
            .map(|part| part.parse::<usize>().ok())
            .collect(),
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
