//! Подготовка декодированного RFC 5322/MIME-текста для IMAP SEARCH.

use super::header_block::{self, HeaderName};
use super::{charset::Charset, encoded_word, parameters::MediaType, tree};

/// Одно декодированное поле outer message header для HEADER/TEXT критериев.
#[derive(Debug)]
pub struct HeaderText {
    pub name: String,
    pub kind: HeaderName,
    pub value: String,
}

/// Неизменяемый текстовый projection одного письма, построенный один раз на SEARCH.
#[derive(Debug)]
pub struct SearchDocument {
    pub headers: Vec<HeaderText>,
    body_text: Vec<String>,
}

impl SearchDocument {
    /// Разбирает заголовки и по запросу строит bounded MIME text projection.
    pub fn parse(raw: &[u8], include_body: bool) -> Result<Self, &'static str> {
        let header_block = header_block::parse(raw)?;
        let headers = header_block
            .fields
            .into_iter()
            .filter_map(|field| {
                let decoded = decode_header_value(&field.value)?;
                let kind = field.kind();
                Some(HeaderText {
                    name: field.name,
                    kind,
                    value: decoded,
                })
            })
            .collect();
        let mut body_text = Vec::new();
        if let Some(Ok(root)) = include_body.then(|| tree::parse(raw)) {
            collect_text(&root, raw, &mut body_text, false);
        }
        Ok(Self { headers, body_text })
    }

    /// Ищет подстроку в декодированных text/* leaf parts без MIME-заголовков.
    pub fn body_contains(&self, needle: &str) -> bool {
        match needle.is_empty() {
            true => true,
            false => self
                .body_text
                .iter()
                .any(|text| contains_case_insensitive_ascii(text, needle)),
        }
    }

    /// Ищет подстроку в decoded RFC 5322 headers и decoded text/* leaf parts.
    pub fn text_contains(&self, needle: &str) -> bool {
        match needle.is_empty() {
            true => true,
            false => {
                self.headers.iter().any(|field| {
                    contains_case_insensitive_ascii(&field.name, needle)
                        || contains_case_insensitive_ascii(&field.value, needle)
                }) || self.body_contains(needle)
            }
        }
    }
}

fn collect_text(
    entity: &tree::Entity,
    raw: &[u8],
    output: &mut Vec<String>,
    include_part_headers: bool,
) {
    if include_part_headers {
        for field in &entity.headers {
            output.push(field.name.clone());
            if let Some(decoded) = decode_header_value(&field.value) {
                output.push(decoded);
            }
        }
    }
    match is_textual(&entity.media_type) {
        true if entity.children.is_empty() => {
            if let Some(text) = decode_text_leaf(entity, raw) {
                output.push(text);
            }
        }
        _ => entity.children.iter().for_each(|child| {
            collect_text(child, raw, output, true);
        }),
    }
}

fn decode_header_value(value: &[u8]) -> Option<String> {
    let value = std::str::from_utf8(value).ok()?;
    encoded_word::decode_header(value).ok()
}

fn is_textual(media_type: &MediaType) -> bool {
    matches!(
        media_type,
        MediaType::TextPlain | MediaType::TextHtml | MediaType::TextOther(_)
    )
}

fn decode_text_leaf(entity: &tree::Entity, raw: &[u8]) -> Option<String> {
    let decoded = entity.decoded_body(raw).ok()?;
    let charset = entity
        .parameters
        .get("charset")
        .map(|name| Charset::parse(name))
        .unwrap_or(Ok(Charset::UsAscii))
        .ok()?;
    charset.decode(&decoded).ok()
}

/// RFC-совместимая минимальная нечувствительность к ASCII-регистру.
pub fn contains_case_insensitive_ascii(haystack: &str, needle: &str) -> bool {
    match needle.is_empty() {
        true => true,
        false => haystack
            .as_bytes()
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle.as_bytes())),
    }
}

#[cfg(test)]
#[path = "search_text_tests.rs"]
mod tests;
