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
        "RFC822.HEADER" => Some(FetchAttribute::Rfc822Header),
        "RFC822.SIZE" => Some(FetchAttribute::Rfc822Size),
        "RFC822.TEXT" => Some(FetchAttribute::Rfc822Text),
        "BODY" => Some(FetchAttribute::BodyStructureBasic),
        "BODYSTRUCTURE" => Some(FetchAttribute::BodyStructure),
        "ENVELOPE" => Some(FetchAttribute::Envelope),
        "FLAGS" => Some(FetchAttribute::Flags),
        "INTERNALDATE" => Some(FetchAttribute::InternalDate),
        "UID" => Some(FetchAttribute::Uid),
        _ => match parse_binary_attribute(token) {
            Some(attribute) => Some(attribute),
            None => match parse_body_attribute(token) {
                Some(attribute) => Some(attribute),
                None => parse_header_fields_attribute(token, tokens),
            },
        },
    }
}

/// Распознаёт три FETCH items RFC 9051 без повторной интерпретации в reactor.
fn parse_binary_attribute(token: &str) -> Option<FetchAttribute> {
    let upper = token.to_ascii_uppercase();
    match upper.strip_prefix("BINARY.SIZE[") {
        Some(value) => {
            let section = value.strip_suffix(']')?;
            Some(FetchAttribute::BinarySize {
                section: parse_binary_path(section)?,
            })
        }
        None => {
            let (peek, value) = match (
                upper.strip_prefix("BINARY.PEEK["),
                upper.strip_prefix("BINARY["),
            ) {
                (Some(value), _) => (true, value),
                (None, Some(value)) => (false, value),
                (None, None) => return None,
            };
            let (section, suffix) = value.split_once(']')?;
            Some(FetchAttribute::Binary {
                section: parse_binary_path(section)?,
                peek,
                partial: match suffix {
                    "" => None,
                    other => Some(parse_partial(other)?),
                },
            })
        }
    }
}

/// Проверяет непустой section-binary path и запрещает нулевые номера частей.
fn parse_binary_path(value: &str) -> Option<Vec<usize>> {
    let path = parse_numeric_path(value)?;
    match (!path.is_empty(), path.iter().all(|number| *number > 0)) {
        (true, true) => Some(path),
        _ => None,
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
                Token::Text(_) | Token::Quoted(_) | Token::Literal(_) | Token::ListStart => {
                    return None;
                }
            }
        },
        Token::Text(_) | Token::Quoted(_) | Token::Literal(_) | Token::ListEnd => return None,
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
    let closing = match tokens.next()? {
        Token::Text(value) => value,
        Token::Quoted(_) | Token::Literal(_) | Token::ListStart | Token::ListEnd => return None,
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
    let offset = offset.parse().ok()?;
    let count = count.parse::<usize>().ok()?;
    match count > 0 {
        true => Some(Partial { offset, count }),
        false => None,
    }
}
