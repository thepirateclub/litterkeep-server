//! Извлечение RFC 3282 language и RFC 2557 location MIME metadata.

use super::header_block::{HeaderField, HeaderName};
use super::language_tag::LanguageTag;

/// Опциональные metadata поля, пригодные для IMAP BODYSTRUCTURE.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentMetadata {
    /// Content-ID в ограниченном однозначном формате `<id-left@id-right>`.
    pub content_id: Option<String>,
    /// Текстовое описание после RFC 2047 decoding.
    pub description: Option<String>,
    /// Исходный Content-MD5 в компактной Base64 форме для IMAP extension slot.
    pub content_md5: Option<String>,
    /// RFC 3282 допускает один или несколько Content-Language tags.
    pub languages: Option<Vec<LanguageTag>>,
    /// URI из единственного Content-Location без преобразования его octets.
    pub location: Option<String>,
}

/// Разбирает metadata независимо: повреждение одного поля не скрывает другое.
pub fn parse(fields: &[HeaderField]) -> ContentMetadata {
    ContentMetadata {
        content_id: super::content_id::parse(fields),
        description: parse_description(fields),
        content_md5: parse_content_md5(fields),
        languages: parse_language_fields(fields),
        location: parse_location_fields(fields),
    }
}

/// Проверяет единственный Content-MD5 как канонический Base64 16-октетного digest.
fn parse_content_md5(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentMd5)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let compact: String = normalized
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    match super::decode_base64(compact.as_bytes()).ok()?.len() {
        16 => Some(compact),
        _ => None,
    }
}

/// Декодирует единственное RFC 2045 Content-Description без изменения raw header.
fn parse_description(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentDescription)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    // Content-Description — unstructured UTF-8 text; Unicode-пробелы здесь являются данными.
    let decoded = super::encoded_word::decode_header(super::wsp::trim(&normalized)).ok()?;
    match decoded.chars().any(char::is_control) {
        true => None,
        false => Some(decoded),
    }
}

/// Принимает только единственное поле с непустым корректным языковым списком.
fn parse_language_fields(fields: &[HeaderField]) -> Option<Vec<LanguageTag>> {
    let field = single_field(fields, HeaderName::ContentLanguage)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let tags = normalized
        .split(',')
        .map(super::wsp::trim)
        .map(LanguageTag::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match tags.is_empty() {
        true => None,
        false => Some(tags),
    }
}

/// Проверяет RFC 3986 URI-reference syntax без преобразования URI octets.
fn parse_location_fields(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentLocation)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    // В CFWS разрешены RFC WSP, а не произвольные Unicode-разделители.
    let location = super::wsp::trim(&normalized);
    match location.is_empty() || !super::uri_reference::is_valid(location) {
        true => None,
        false => Some(location.to_owned()),
    }
}

/// Избегает промежуточного вектора и считает повторы одним проходом.
fn single_field(fields: &[HeaderField], name: HeaderName) -> Option<&HeaderField> {
    let mut matching = fields.iter().filter(|field| field.kind() == name);
    let field = matching.next()?;
    match matching.next() {
        Some(_) => None,
        None => Some(field),
    }
}

#[cfg(test)]
#[path = "content_metadata_tests.rs"]
mod tests;
