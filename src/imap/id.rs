//! Проверка приватного IMAP ID extension по RFC 2971.

use std::collections::HashSet;

use super::tokenizer::Token;

const MAX_FIELDS: usize = 30;
const MAX_FIELD_OCTETS: usize = 30;
const MAX_VALUE_OCTETS: usize = 1024;

/// Проверяет ID параметры, не сохраняя и не журналируя данные клиента.
pub(crate) fn parse(mut tokens: impl Iterator<Item = Token>) -> bool {
    match tokens.next() {
        Some(Token::Text(value)) if value.eq_ignore_ascii_case("NIL") => tokens.next().is_none(),
        Some(Token::ListStart) => parse_list(&mut tokens),
        _ => false,
    }
}

/// Проверяет пары, уникальность полей и ограничения размера RFC 2971.
fn parse_list(tokens: &mut impl Iterator<Item = Token>) -> bool {
    let mut names = HashSet::new();
    loop {
        match tokens.next() {
            Some(Token::ListEnd) => return tokens.next().is_none(),
            Some(field) => {
                let Some(field) = string_bytes(field) else {
                    return false;
                };
                let Some(value) = tokens.next() else {
                    return false;
                };
                if !valid_field(&field, &mut names) || !valid_value(value) {
                    return false;
                }
            }
            None => return false,
        }
    }
}

/// Извлекает IMAP string production: quoted-string или literal, не atom.
fn string_bytes(token: Token) -> Option<Vec<u8>> {
    match token {
        Token::Quoted(value) => Some(value.into_bytes()),
        Token::Literal(value) => Some(value),
        Token::Text(_) | Token::ListStart | Token::ListEnd => None,
    }
}

/// Валидирует имя поля и запрещает дубли без учёта ASCII-регистра.
fn valid_field(field: &[u8], names: &mut HashSet<Vec<u8>>) -> bool {
    if field.len() > MAX_FIELD_OCTETS || names.len() >= MAX_FIELDS {
        return false;
    }
    let normalized = field.iter().map(u8::to_ascii_lowercase).collect();
    names.insert(normalized)
}

/// Значение есть string либо специальный atom NIL, ограниченный 1024 octets.
fn valid_value(value: Token) -> bool {
    match value {
        Token::Text(value) => value.eq_ignore_ascii_case("NIL"),
        string => string_bytes(string)
            .map(|bytes| bytes.len() <= MAX_VALUE_OCTETS)
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::tokenizer::tokenize;

    fn valid(command: &str) -> bool {
        let Ok(tokens) = tokenize(command) else {
            return false;
        };
        let mut tokens = tokens.into_iter();
        tokens.next();
        tokens.next();
        parse(tokens)
    }

    /// RFC 2971 NIL, пустой список и string/NIL пары допустимы.
    #[test]
    fn accepts_nil_empty_list_and_string_pairs() {
        assert!(valid("A ID NIL\r\n"));
        assert!(valid("A ID ()\r\n"));
        assert!(valid("A ID (\"name\" \"client\" \"vendor\" NIL)\r\n"));
    }

    /// Atom не является string полем, список требует целые пары и без хвоста.
    #[test]
    fn rejects_malformed_pair_lists() {
        assert!(!valid("A ID (name \"client\")\r\n"));
        assert!(!valid("A ID (\"name\")\r\n"));
        assert!(!valid("A ID (\"name\" \"client\"\r\n"));
        assert!(!valid("A ID NIL extra\r\n"));
    }

    /// RFC 2971 limits fields, values, field count, and duplicate names.
    #[test]
    fn enforces_field_value_count_and_uniqueness_limits() {
        let long_field = format!("A ID (\"{}\" NIL)\r\n", "f".repeat(31));
        let long_value = format!("A ID (\"f\" \"{}\")\r\n", "v".repeat(1025));
        assert!(!valid(&long_field));
        assert!(!valid(&long_value));
        assert!(!valid("A ID (\"name\" NIL \"NAME\" NIL)\r\n"));
        let fields = (0..31)
            .map(|index| format!("\"f{index}\" NIL"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!valid(&format!("A ID ({fields})\r\n")));
    }
}
