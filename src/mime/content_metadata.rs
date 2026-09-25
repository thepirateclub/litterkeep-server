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
    /// RFC 3282 допускает один или несколько Content-Language tags.
    pub languages: Option<Vec<LanguageTag>>,
    /// URI из единственного Content-Location без преобразования его octets.
    pub location: Option<String>,
}

/// Разбирает metadata независимо: повреждение одного поля не скрывает другое.
pub fn parse(fields: &[HeaderField]) -> ContentMetadata {
    ContentMetadata {
        content_id: parse_content_id(fields),
        description: parse_description(fields),
        languages: parse_language_fields(fields),
        location: parse_location_fields(fields),
    }
}

/// Принимает единственный видимый ASCII Content-ID без folding и комментариев.
fn parse_content_id(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentId)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let identifier = normalized.trim();
    let inner = identifier.strip_prefix('<')?.strip_suffix('>')?;
    let (left, right) = inner.split_once('@')?;
    match valid_dot_atom(left) && valid_dot_atom(right) {
        true => Some(identifier.to_owned()),
        false => None,
    }
}

/// Проверяет поддерживаемое подмножество RFC 5322 dot-atom без quoted forms.
fn valid_dot_atom(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|atom| !atom.is_empty() && atom.bytes().all(is_atext_or_dot))
}

fn is_atext_or_dot(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'/'
                | b'='
                | b'?'
                | b'^'
                | b'_'
                | b'`'
                | b'{'
                | b'|'
                | b'}'
                | b'~'
                | b'.'
        )
}

/// Декодирует единственное RFC 2045 Content-Description без изменения raw header.
fn parse_description(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentDescription)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let decoded = super::encoded_word::decode_header(normalized.trim()).ok()?;
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
        .map(str::trim)
        .map(LanguageTag::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match tags.is_empty() {
        true => None,
        false => Some(tags),
    }
}

/// Принимает URI как непрозрачный текст, сохраняя допустимые схемы и формы.
fn parse_location_fields(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentLocation)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let location = normalized.trim();
    match location.is_empty() || location.chars().any(char::is_control) {
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
