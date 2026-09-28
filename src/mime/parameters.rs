//! Разбор MIME media type и параметров, включая RFC 2231 continuations.

use super::language_tag::LanguageTag;
use std::collections::{BTreeMap, HashMap};

/// Известный media type либо сохранённое неизвестное значение.
#[derive(Debug, Eq, PartialEq)]
pub enum MediaType {
    TextPlain,
    TextHtml,
    TextOther(String),
    MultipartMixed,
    MultipartAlternative,
    MultipartRelated,
    MultipartDigest,
    MultipartOther(String),
    MessageRfc822,
    /// Международное вложенное сообщение с UTF-8 заголовками по RFC 6532.
    MessageGlobal,
    MessageOther(String),
    ApplicationOctetStream,
    Other(String),
}

impl MediaType {
    /// Возвращает IMAP type/subtype без повторного строкового распознавания.
    pub fn type_and_subtype(&self) -> (&str, &str) {
        match self {
            Self::TextPlain => ("TEXT", "PLAIN"),
            Self::TextHtml => ("TEXT", "HTML"),
            Self::TextOther(subtype) => ("TEXT", subtype),
            Self::MultipartMixed => ("MULTIPART", "MIXED"),
            Self::MultipartAlternative => ("MULTIPART", "ALTERNATIVE"),
            Self::MultipartRelated => ("MULTIPART", "RELATED"),
            Self::MultipartDigest => ("MULTIPART", "DIGEST"),
            Self::MultipartOther(subtype) => ("MULTIPART", subtype),
            Self::MessageRfc822 => ("MESSAGE", "RFC822"),
            Self::MessageGlobal => ("MESSAGE", "GLOBAL"),
            Self::MessageOther(subtype) => ("MESSAGE", subtype),
            Self::ApplicationOctetStream => ("APPLICATION", "OCTET-STREAM"),
            Self::Other(value) => value.split_once('/').unwrap_or((value, "UNKNOWN")),
        }
    }
}

/// Structured MIME value после разбора параметров.
#[derive(Debug, Eq, PartialEq)]
pub struct StructuredValue {
    pub media_type: MediaType,
    pub parameters: BTreeMap<String, String>,
    /// Языковые подсказки RFC 2231 для тех decoded parameter, где они заданы.
    pub parameter_languages: BTreeMap<String, LanguageTag>,
}

/// Разбирает Content-Type или Content-Disposition без потери неизвестного типа.
pub fn parse(input: &str) -> Result<StructuredValue, &'static str> {
    let normalized = super::structured_comments::remove_preserving_adjacency(input)?;
    let sections = split_sections(&normalized)?;
    let mut iterator = sections.into_iter();
    let media_type = parse_media_type(super::wsp::trim(
        iterator.next().ok_or("missing MIME value")?,
    ))?;
    let mut grouped: HashMap<String, Vec<super::parameter_join::Segment>> = HashMap::new();
    for section in iterator {
        let section = trim_comment_wsp(section);
        if section.is_empty() {
            continue;
        }
        super::parameter_separator::validate(section)?;
        let normalized_section = normalize_parameter_section(section)?;
        let (raw_name, raw_value) = normalized_section
            .split_once('=')
            .ok_or("parameter without value")?;
        let raw_name = super::wsp::trim(raw_name);
        let raw_value = super::wsp::trim(raw_value);
        let (base, index, encoded) = parameter_name(raw_name)?;
        let value = unquote(raw_value)?;
        grouped
            .entry(base)
            .or_default()
            .push(super::parameter_join::Segment::new(index, encoded, value));
    }
    let mut parameters = BTreeMap::new();
    let mut parameter_languages = BTreeMap::new();
    for (name, segments) in grouped {
        let (value, language) = super::parameter_join::join_segments(segments)?;
        parameters.insert(name.clone(), value);
        if let Some(language) = language {
            parameter_languages.insert(name, language);
        }
    }
    Ok(StructuredValue {
        media_type,
        parameters,
        parameter_languages,
    })
}

/// Удаляет comments только там, где RFC structured syntax трактует их как WSP.
fn normalize_parameter_section(section: &str) -> Result<String, &'static str> {
    let mut normalized = String::new();
    let mut depth = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    for character in section.chars() {
        match (depth, quoted, escaped, character) {
            (_, true, true, _) => {
                normalized.push(character);
                escaped = false;
            }
            (0, true, false, '\\') => {
                normalized.push(character);
                escaped = true;
            }
            (0, true, false, '"') => {
                normalized.push(character);
                quoted = false;
            }
            (0, true, false, _) => normalized.push(character),
            (0, false, _, '"') => {
                normalized.push(character);
                quoted = true;
            }
            (0, false, _, '(') => {
                normalized.push(' ');
                depth = 1;
            }
            (0, false, _, _) => normalized.push(character),
            (_, false, _, '(') => depth = depth.saturating_add(1),
            (_, false, _, ')') => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    match (depth, quoted, escaped) {
        (0, false, false) => Ok(normalized),
        _ => Err("malformed MIME parameter syntax"),
    }
}

/// Снимает пробелы до и после секции, не удаляя comment-разделители.
fn trim_comment_wsp(value: &str) -> &str {
    let trimmed = value.trim_start_matches([' ', '\t']);
    trimmed.trim_end_matches([' ', '\t'])
}

fn split_sections(input: &str) -> Result<Vec<&str>, &'static str> {
    let mut result = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0_usize;
    for (position, character) in input.char_indices() {
        match (quoted, escaped, character) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (_, false, '"') => quoted = !quoted,
            (false, false, ';') => {
                result.push(
                    input
                        .get(start..position)
                        .ok_or("invalid parameter boundary")?,
                );
                start = position.saturating_add(character.len_utf8());
            }
            _ => {}
        }
    }
    match (quoted, escaped) {
        (true, _) | (false, true) => return Err("unterminated quoted parameter"),
        (false, false) => {}
    }
    result.push(input.get(start..).ok_or("invalid parameter tail")?);
    Ok(result)
}

/// Разбирает обычный атрибут, RFC 2231 extended-форму или её секцию.
///
/// Секция со значением 0 начинает continuation; последующие номера должны
/// быть каноническими положительными числами без ведущих нулей.
fn parameter_name(name: &str) -> Result<(String, Option<usize>, bool), &'static str> {
    let lower = name.to_ascii_lowercase();
    let encoded = lower.ends_with('*');
    let stem = lower.strip_suffix('*').unwrap_or(&lower);
    let Some((base, suffix)) = stem.rsplit_once('*') else {
        if !attribute_token(stem) {
            return Err("invalid MIME parameter attribute");
        }
        return Ok((stem.to_owned(), None, encoded));
    };
    if !attribute_token(base) {
        return Err("invalid MIME parameter attribute");
    }
    let valid_section = match (suffix.bytes().next(), suffix.len()) {
        (Some(b'0'), 1) => true,
        (Some(b'1'..=b'9'), _) => suffix.bytes().all(|byte| byte.is_ascii_digit()),
        _ => false,
    };
    if !valid_section {
        return Err("invalid continuation index");
    }
    let index = suffix
        .parse::<usize>()
        .map_err(|_| "invalid continuation index")?;
    Ok((base.to_owned(), Some(index), encoded))
}

/// Проверяет имя MIME-атрибута с ограничениями RFC 2231.
///
/// Звёздочка, апостроф и процент зарезервированы расширенной грамматикой;
/// остальные ограничения token наследуются из RFC 2045.
fn attribute_token(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(attribute_octet)
}

/// Проверяет MIME token либо снимает кавычки и quoted-pair у значения.
///
/// Пустое значение допустимо только в кавычках; неэкранированная внутренняя
/// кавычка и оборванная escape-последовательность являются ошибкой.
fn unquote(value: &str) -> Result<String, &'static str> {
    let Some(inner) = value.strip_prefix('"') else {
        return match media_token(value) {
            true => Ok(value.to_owned()),
            false => Err("invalid MIME parameter token"),
        };
    };
    let inner = inner.strip_suffix('"').ok_or("unterminated quoted value")?;
    let mut output = String::new();
    let mut escaped = false;
    for character in inner.chars() {
        match (escaped, character) {
            (true, _) => {
                output.push(character);
                escaped = false;
            }
            (false, '\\') => escaped = true,
            (false, '"') => return Err("unescaped quote in MIME parameter"),
            (false, _) => output.push(character),
        }
    }
    if escaped {
        return Err("trailing quoted escape");
    }
    Ok(output)
}

/// Разбирает пару `type/subtype` и классифицирует известные MIME-типы.
///
/// Неизвестная, но корректная пара сохраняется для последующих протокольных
/// ответов; синтаксически повреждённая пара не маскируется как `Other`.
fn parse_media_type(value: &str) -> Result<MediaType, &'static str> {
    let normalized = value.to_ascii_lowercase();
    let mut sections = normalized.split('/');
    let Some(kind) = sections.next().map(super::wsp::trim) else {
        return Err("missing MIME type");
    };
    let Some(subtype) = sections.next().map(super::wsp::trim) else {
        return Err("missing MIME subtype");
    };
    if sections.next().is_some() || !media_token(kind) || !media_token(subtype) {
        return Err("invalid MIME type or subtype token");
    }
    Ok(match (kind, subtype) {
        ("text", "plain") => MediaType::TextPlain,
        ("text", "html") => MediaType::TextHtml,
        ("multipart", "mixed") => MediaType::MultipartMixed,
        ("multipart", "alternative") => MediaType::MultipartAlternative,
        ("multipart", "related") => MediaType::MultipartRelated,
        ("multipart", "digest") => MediaType::MultipartDigest,
        ("message", "rfc822") => MediaType::MessageRfc822,
        ("message", "global") => MediaType::MessageGlobal,
        ("application", "octet-stream") => MediaType::ApplicationOctetStream,
        ("text", subtype) => MediaType::TextOther(subtype.to_owned()),
        ("multipart", subtype) => MediaType::MultipartOther(subtype.to_owned()),
        ("message", subtype) => MediaType::MessageOther(subtype.to_owned()),
        _ => MediaType::Other(format!("{kind}/{subtype}")),
    })
}

/// Проверяет ASCII token без пробелов, управляющих октетов и MIME tspecials.
///
/// Длина измеряется в байтах, поскольку RFC определяет type/subtype как
/// последовательности ASCII-октетов, а не Unicode-символов.
pub(super) fn media_token(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(media_token_octet)
}

/// Проверяет один RFC 2045 token-октет, не обращаясь к позиции массива.
pub(super) fn media_token_octet(byte: u8) -> bool {
    matches!(byte, 33..=126) && !b"()<>@,;:\\\"/[]?=".contains(&byte)
}

/// Проверяет RFC 2231 attribute-char на основе общих MIME tspecials.
pub(super) fn attribute_octet(byte: u8) -> bool {
    media_token_octet(byte) && !matches!(byte, b'*' | b'\'' | b'%')
}
