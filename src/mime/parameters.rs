//! Разбор MIME media type и параметров, включая RFC 2231 continuations.

use super::charset::Charset;
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
    MessageRfc822,
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
            Self::MessageRfc822 => ("MESSAGE", "RFC822"),
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
    let sections = split_sections(input)?;
    let mut iterator = sections.into_iter();
    let media_type = parse_media_type(iterator.next().ok_or("missing MIME value")?.trim());
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

fn parameter_name(name: &str) -> Result<(String, Option<usize>, bool), &'static str> {
    let lower = name.to_ascii_lowercase();
    let encoded = lower.ends_with('*');
    let stem = lower.strip_suffix('*').unwrap_or(&lower);
    let Some((base, suffix)) = stem.rsplit_once('*') else {
        return Ok((stem.to_owned(), None, encoded));
    };
    let index = suffix
        .parse::<usize>()
        .map_err(|_| "invalid continuation index")?;
    if base.is_empty() {
        return Err("empty parameter name");
    }
    Ok((base.to_owned(), Some(index), encoded))
}

fn unquote(value: &str) -> Result<String, &'static str> {
    let Some(inner) = value.strip_prefix('"') else {
        return Ok(value.to_owned());
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
        let (selected, payload) = match (expected, segment.encoded) {
            (0, true) => extended_parts(&segment.value)?,
            (_, true) => (None, segment.value.as_str()),
            (_, false) => (None, segment.value.as_str()),
        };
        if selected.is_some() {
            charset = selected;
        }
        match segment.encoded {
            true => bytes.extend(percent_decode(payload)?),
            false => bytes.extend_from_slice(payload.as_bytes()),
        }
    }
    match charset {
        Some(charset) => charset.decode(&bytes),
        None => String::from_utf8(bytes).map_err(|_| "continuation is not UTF-8"),
    }
}

fn decode_extended(value: &str) -> Result<String, &'static str> {
    let (charset, payload) = extended_parts(value)?;
    charset
        .ok_or("missing extended charset")?
        .decode(&percent_decode(payload)?)
}

fn extended_parts(value: &str) -> Result<(Option<Charset>, &str), &'static str> {
    let (charset, rest) = value
        .split_once('\'')
        .ok_or("missing extended charset separator")?;
    let (_, payload) = rest
        .split_once('\'')
        .ok_or("missing extended language separator")?;
    Ok((Some(Charset::parse(charset)?), payload))
}

fn percent_decode(value: &str) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(value.len());
    let mut cursor = 0_usize;
    let bytes = value.as_bytes();
    while let Some(byte) = bytes.get(cursor).copied() {
        match byte {
            b'%' => {
                let high = bytes.get(cursor.saturating_add(1)).copied().and_then(hex);
                let low = bytes.get(cursor.saturating_add(2)).copied().and_then(hex);
                let (Some(high), Some(low)) = (high, low) else {
                    return Err("invalid percent escape");
                };
                output.push((high << 4) | low);
                cursor = cursor.saturating_add(3);
            }
            _ => {
                output.push(byte);
                cursor = cursor.saturating_add(1);
            }
        }
    }
    Ok(output)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn parse_media_type(value: &str) -> MediaType {
    let normalized = value.to_ascii_lowercase();
    match normalized.as_str() {
        "text/plain" => MediaType::TextPlain,
        "text/html" => MediaType::TextHtml,
        "multipart/mixed" => MediaType::MultipartMixed,
        "multipart/alternative" => MediaType::MultipartAlternative,
        "multipart/related" => MediaType::MultipartRelated,
        "message/rfc822" => MediaType::MessageRfc822,
        "application/octet-stream" => MediaType::ApplicationOctetStream,
        _ => match normalized.split_once('/') {
            Some(("text", subtype)) => MediaType::TextOther(subtype.to_owned()),
            _ => MediaType::Other(normalized),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_boundary_and_filename_continuation() {
        let value = "application/octet-stream; boundary=\"a;b\"; filename*0*=UTF-8''%D1%81%D1%87; filename*1*=%D1%91%D1%82.txt";
        let parsed = parse(value).expect("structured MIME value");
        assert_eq!(parsed.media_type, MediaType::ApplicationOctetStream);
        assert_eq!(
            parsed.parameters.get("boundary").map(String::as_str),
            Some("a;b")
        );
        assert_eq!(
            parsed.parameters.get("filename").map(String::as_str),
            Some("счёт.txt")
        );
    }

    #[test]
    fn rejects_gapped_continuation_and_unknown_charset() {
        assert!(parse("text/plain; name*1=x").is_err());
        assert!(parse("text/plain; name*=koi8-r''text").is_err());
    }
}
