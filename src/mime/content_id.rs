//! RFC 2045 Content-ID извлечение и безопасная RFC 5322 нормализация.

use super::header_block::{HeaderField, HeaderName};

/// Возвращает единственный валидный Content-ID без семантически пустого CFWS.
pub(super) fn parse(fields: &[HeaderField]) -> Option<String> {
    let field = single_field(fields)?;
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

/// Отбрасывает повторные Content-ID, не угадывая, какое значение выбрал автор.
fn single_field(fields: &[HeaderField]) -> Option<&HeaderField> {
    let mut matching = fields
        .iter()
        .filter(|field| field.kind() == HeaderName::ContentId);
    let field = matching.next()?;
    match matching.next() {
        Some(_) => None,
        None => Some(field),
    }
}

/// Ищет разделитель пары вне quoted-string и domain-literal.
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

/// Принимает local-part и obsolete dot-separated quoted/atom words.
fn normalize_id_left(value: &str) -> Option<String> {
    normalize_local_part(value.trim_matches([' ', '\t']))
}

/// Нормализует `word *("." word)`, не разделяя точки внутри кавычек.
fn normalize_local_part(value: &str) -> Option<String> {
    let mut state = LocalPartState::Plain;
    let mut word = String::new();
    let mut output = String::new();
    for character in value.chars() {
        state = match (state, character) {
            (LocalPartState::Plain, '"') => push_and(LocalPartState::Quoted, &mut word, character),
            (LocalPartState::Plain, '.') => {
                append_local_word(&mut output, &mut word)?;
                output.push('.');
                LocalPartState::Plain
            }
            (LocalPartState::Plain, character) => {
                push_and(LocalPartState::Plain, &mut word, character)
            }
            (LocalPartState::Quoted, '\\') => {
                push_and(LocalPartState::QuotedEscape, &mut word, character)
            }
            (LocalPartState::Quoted, '"') => push_and(LocalPartState::Plain, &mut word, character),
            (LocalPartState::Quoted, character) => {
                push_and(LocalPartState::Quoted, &mut word, character)
            }
            (LocalPartState::QuotedEscape, character) if !character.is_control() => {
                push_and(LocalPartState::Quoted, &mut word, character)
            }
            (LocalPartState::QuotedEscape, _) => return None,
        };
    }
    match state {
        LocalPartState::Plain => append_local_word(&mut output, &mut word).map(|()| output),
        LocalPartState::Quoted | LocalPartState::QuotedEscape => None,
    }
}

/// Добавляет один atom или quoted-string, удаляя CFWS только по его краям.
fn append_local_word(output: &mut String, word: &mut String) -> Option<()> {
    let value = word.trim_matches([' ', '\t']);
    match value
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    {
        Some(text) if valid_quoted_text(text) => output.push_str(value),
        Some(_) => return None,
        None if valid_atext_atom(value) => output.push_str(value),
        None => return None,
    }
    word.clear();
    Some(())
}

/// Переводит один символ автомата, добавляя его к текущему слову.
fn push_and(state: LocalPartState, word: &mut String, character: char) -> LocalPartState {
    word.push(character);
    state
}

/// Проверяет dot-atom либо no-fold-literal для id-right.
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
        None => normalize_domain(value),
    }
}

/// Убирает CFWS вокруг obsolete domain atoms и точек между ними.
fn normalize_domain(value: &str) -> Option<String> {
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

/// Проверяет непустой atom RFC 5322/RFC 6532.
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

/// Проверяет quoted-string без folding и управляющих символов.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalPartState {
    Plain,
    Quoted,
    QuotedEscape,
}
