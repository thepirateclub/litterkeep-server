//! Разбор MIME media type и параметров, включая RFC 2231 continuations.

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
}

#[derive(Debug)]
struct Segment {
    index: Option<usize>,
    encoded: bool,
    value: String,
}

/// Разбирает Content-Type или Content-Disposition без потери неизвестного типа.
pub fn parse(input: &str) -> Result<StructuredValue, &'static str> {
    let normalized = super::structured_comments::remove(input)?;
    let sections = split_sections(&normalized)?;
    let mut iterator = sections.into_iter();
    let media_type = parse_media_type(iterator.next().ok_or("missing MIME value")?.trim())?;
    let mut grouped: HashMap<String, Vec<Segment>> = HashMap::new();
    for section in iterator {
        let (raw_name, raw_value) = section.split_once('=').ok_or("parameter without value")?;
        let (base, index, encoded) = parameter_name(raw_name.trim())?;
        let value = unquote(raw_value.trim())?;
        grouped.entry(base).or_default().push(Segment {
            index,
            encoded,
            value,
        });
    }
    let mut parameters = BTreeMap::new();
    for (name, segments) in grouped {
        parameters.insert(name, join_segments(segments)?);
    }
    Ok(StructuredValue {
        media_type,
        parameters,
    })
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

fn join_segments(mut segments: Vec<Segment>) -> Result<String, &'static str> {
    match segments.iter().all(|segment| segment.index.is_none()) {
        true if segments.len() == 1 => decode_single(segments.pop().expect("one segment")),
        true => Err("duplicate MIME parameter"),
        false => join_continuations(segments),
    }
}

fn decode_single(segment: Segment) -> Result<String, &'static str> {
    match segment.encoded {
        true => decode_extended(&segment.value),
        false => Ok(segment.value),
    }
}

fn join_continuations(mut segments: Vec<Segment>) -> Result<String, &'static str> {
    segments.sort_by_key(|segment| segment.index);
    let mut bytes = Vec::new();
    let mut charset = None;
    for (expected, segment) in segments.into_iter().enumerate() {
        if segment.index != Some(expected) {
            return Err("non-contiguous MIME continuation");
        }
        let (selected, decoded) = match (expected, segment.encoded) {
            (0, true) => super::parameter_extended::initial(&segment.value)?,
            (_, true) => (
                None,
                super::parameter_extended::continuation(&segment.value)?,
            ),
            (_, false) => (None, segment.value.into_bytes()),
        };
        match (expected, segment.encoded, selected) {
            (0, true, None) => return Err("missing extended charset"),
            (_, _, Some(selected)) => charset = Some(selected),
            _ => {}
        }
        bytes.extend(decoded);
    }
    match charset {
        Some(charset) => charset.decode(&bytes),
        None => String::from_utf8(bytes).map_err(|_| "continuation is not UTF-8"),
    }
}

fn decode_extended(value: &str) -> Result<String, &'static str> {
    let (charset, bytes) = super::parameter_extended::initial(value)?;
    charset.ok_or("missing extended charset")?.decode(&bytes)
}

/// Разбирает пару `type/subtype` и классифицирует известные MIME-типы.
///
/// Неизвестная, но корректная пара сохраняется для последующих протокольных
/// ответов; синтаксически повреждённая пара не маскируется как `Other`.
fn parse_media_type(value: &str) -> Result<MediaType, &'static str> {
    let normalized = value.to_ascii_lowercase();
    let mut sections = normalized.split('/');
    let Some(kind) = sections.next().map(str::trim) else {
        return Err("missing MIME type");
    };
    let Some(subtype) = sections.next().map(str::trim) else {
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
