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
        content_id: parse_content_id(fields),
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

/// Принимает единственный Content-ID в поддерживаемой RFC 5322/6532 грамматике.
fn parse_content_id(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentId)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let identifier = normalized.trim_matches([' ', '\t']);
    let inner = identifier.strip_prefix('<')?.strip_suffix('>')?;
    let separator = separator_at(inner)?;
    let (left, right_with_separator) = inner.split_at(separator);
    let right = right_with_separator.get(1..)?;
    match (normalize_id_left(left), normalize_id_right(right)) {
        (Some(left), Some(right)) => Some(format!("<{left}@{right}>")),
        _ => None,
    }
}

/// Находит единственный разделитель вне quoted-string и domain-literal.
fn separator_at(value: &str) -> Option<usize> {
    let mut state = IdentifierState::Plain;
    let mut separator = None;
    for (offset, character) in value.char_indices() {
        state = match (state, character) {
            (IdentifierState::Plain, '"') => IdentifierState::Quoted,
            (IdentifierState::Plain, '[') => IdentifierState::DomainLiteral,
            (IdentifierState::Plain, '@') if separator.is_none() => {
                separator = Some(offset);
                IdentifierState::Plain
            }
            (IdentifierState::Plain, '@') => return None,
            (IdentifierState::Quoted, '\\') => IdentifierState::QuotedEscape,
            (IdentifierState::Quoted, '"') => IdentifierState::Plain,
            (IdentifierState::Quoted, _) => IdentifierState::Quoted,
            (IdentifierState::QuotedEscape, character) if !character.is_control() => {
                IdentifierState::Quoted
            }
            (IdentifierState::DomainLiteral, '\\') => IdentifierState::DomainEscape,
            (IdentifierState::DomainLiteral, ']') => IdentifierState::Plain,
            (IdentifierState::DomainLiteral, '[') => return None,
            (IdentifierState::DomainLiteral, character) if !character.is_control() => {
                IdentifierState::DomainLiteral
            }
            (IdentifierState::DomainEscape, character) if !character.is_control() => {
                IdentifierState::DomainLiteral
            }
            (IdentifierState::QuotedEscape | IdentifierState::DomainEscape, _) => return None,
            (IdentifierState::Plain, ']') => return None,
            (IdentifierState::Plain, '\t') => IdentifierState::Plain,
            (IdentifierState::Plain, character) if !character.is_control() => {
                IdentifierState::Plain
            }
            _ => return None,
        };
    }
    match (state, separator) {
        (IdentifierState::Plain, Some(offset)) => Some(offset),
        _ => None,
    }
}

/// Поддерживает RFC dot-atom либо quoted-string для id-left.
fn normalize_id_left(value: &str) -> Option<String> {
    let value = value.trim_matches([' ', '\t']);
    match value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
    {
        Some(inner) => match valid_quoted_text(inner) {
            true => Some(value.to_owned()),
            false => None,
        },
        None => normalize_dot_atom(value),
    }
}

/// Поддерживает RFC dot-atom либо no-fold-literal для id-right.
fn normalize_id_right(value: &str) -> Option<String> {
    let value = value.trim_matches([' ', '\t']);
    match value
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    {
        Some(inner) => match valid_domain_literal(inner) {
            true => Some(value.to_owned()),
            false => None,
        },
        None => normalize_dot_atom(value),
    }
}

/// Убирает семантически невидимые CFWS вокруг точек obsolete id-формы.
fn normalize_dot_atom(value: &str) -> Option<String> {
    let mut normalized = String::new();
    let mut first = true;
    for segment in value.split('.') {
        let atom = segment.trim_matches([' ', '\t']);
        if !valid_atext_atom(atom) {
            return None;
        }
        match first {
            true => first = false,
            false => normalized.push('.'),
        }
        normalized.push_str(atom);
    }
    Some(normalized)
}

fn valid_atext_atom(value: &str) -> bool {
    match value.chars().next() {
        Some(_) => value.chars().all(is_atext),
        None => false,
    }
}

fn is_atext(character: char) -> bool {
    match character {
        character if character.is_ascii_alphanumeric() => true,
        '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '/' | '=' | '?' | '^' | '_'
        | '`' | '{' | '|' | '}' | '~' => true,
        character if !character.is_ascii() => !character.is_control(),
        _ => false,
    }
}

/// Проверяет quoted-string/literal payload без CR/LF folding и управляющих символов.
fn valid_quoted_text(value: &str) -> bool {
    let mut escaped = false;
    for character in value.chars() {
        match (escaped, character) {
            (true, character) if !character.is_control() => escaped = false,
            (true, _) => return false,
            (false, '\\') => escaped = true,
            (false, character) if !character.is_control() && character != '"' => {}
            _ => return false,
        }
    }
    !escaped
}

/// Проверяет dtext/domain quoted-pair без вложенных квадратных скобок.
fn valid_domain_literal(value: &str) -> bool {
    let mut escaped = false;
    for character in value.chars() {
        match (escaped, character) {
            (true, character) if !character.is_control() => escaped = false,
            (true, _) => return false,
            (false, '\\') => escaped = true,
            (false, '[' | ']') => return false,
            (false, character) if !character.is_control() => {}
            _ => return false,
        }
    }
    !escaped
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentifierState {
    Plain,
    Quoted,
    QuotedEscape,
    DomainLiteral,
    DomainEscape,
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

/// Проверяет RFC 3986 URI-reference syntax без преобразования URI octets.
fn parse_location_fields(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields, HeaderName::ContentLocation)?;
    let value = std::str::from_utf8(&field.value).ok()?;
    let normalized = super::structured_comments::remove(value).ok()?;
    let location = normalized.trim();
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
