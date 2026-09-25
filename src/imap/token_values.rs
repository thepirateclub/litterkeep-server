//! Контекстная проверка строковых production IMAP после лексического разбора.

use super::tokenizer::Token;

/// Проверяет command tag по `ASTRING-CHAR` с исключением RFC для plus.
pub(crate) fn valid_tag(tag: &str) -> bool {
    match tag.is_empty() {
        true => false,
        false => tag.bytes().all(valid_tag_byte),
    }
}

/// Извлекает astring только из корректного atom либо уже проверенной кавычки.
pub(crate) fn astring_value(token: Token) -> Option<String> {
    match token {
        Token::Text(value) if valid_astring_atom(&value) => Some(value),
        Token::Quoted(value) => Some(value),
        Token::Text(_) | Token::ListStart | Token::ListEnd => None,
    }
}

/// Извлекает только корректный IMAP atom из лексического токена.
pub(crate) fn atom_value(token: Token) -> Option<String> {
    match token {
        Token::Text(value) if valid_atom(&value) => Some(value),
        Token::Text(_) | Token::Quoted(_) | Token::ListStart | Token::ListEnd => None,
    }
}

/// Проверяет `ATOM-CHAR`, не включая расширения astring и LIST pattern.
fn valid_atom(value: &str) -> bool {
    match value.is_empty() {
        true => false,
        false => value.bytes().all(valid_atom_byte),
    }
}

/// Исключает все atom-specials из видимого ASCII диапазона.
fn valid_atom_byte(value: u8) -> bool {
    match (value.is_ascii_graphic(), value) {
        (true, b'(' | b')' | b'{' | b'}' | b'%' | b'*' | b']' | b'"' | b'\\') => false,
        (true, _) => true,
        (false, _) => false,
    }
}

/// Проверяет unquoted часть `ASTRING-CHAR = ATOM-CHAR / resp-specials`.
fn valid_astring_atom(value: &str) -> bool {
    match value.is_empty() {
        true => false,
        false => value.bytes().all(valid_astring_byte),
    }
}

/// Разрешает response-special `]`, но отвергает остальные atom-specials.
fn valid_astring_byte(value: u8) -> bool {
    match (value.is_ascii_graphic(), value) {
        (true, b'(' | b')' | b'{' | b'}' | b'%' | b'*' | b'"' | b'\\') => false,
        (true, _) => true,
        (false, _) => false,
    }
}

/// Проверяет один ASCII octet production `tag` из RFC 9051.
fn valid_tag_byte(value: u8) -> bool {
    match (value.is_ascii_graphic(), value) {
        (true, b'(' | b')' | b'{' | b'}' | b'%' | b'*' | b'+' | b'"' | b'\\') => false,
        (true, _) => true,
        (false, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tag допускает resp-special `]`, но исключает plus по отдельной production.
    #[test]
    fn tag_uses_astring_char_except_plus() {
        assert!(valid_tag("A]7"));
        assert!(!valid_tag("A+7"));
        assert!(!valid_tag("A]7\t"));
        assert_eq!(atom_value(Token::Text("A*".into())), None);
    }
}
