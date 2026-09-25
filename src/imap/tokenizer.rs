//! Лексический анализ одной завершённой IMAP-командной строки.
//!
//! Лексер проходит вход один раз и не копирует его многократно. Кавычки и
//! экранирование обрабатываются здесь, поэтому предметный parser работает с
//! готовыми токенами и не восстанавливает синтаксис из строковых эвристик.

/// Синтаксический токен IMAP-команды.
#[derive(Debug, Eq, PartialEq)]
pub enum Token {
    /// Неэкранированный протокольный atom.
    Text(String),
    /// Quoted string с уже обработанными escape-последовательностями.
    Quoted(String),
    /// Открывающая круглая скобка списка.
    ListStart,
    /// Закрывающая круглая скобка списка.
    ListEnd,
}

/// Разбирает одну CRLF-строку за `O(n)` времени с ограничением числа токенов.
pub fn tokenize(line: &str) -> Result<Vec<Token>, &'static str> {
    tokenize_internal(line, None)
}

/// Разбирает команду, в которой NUL-placeholder обозначает принятый SEARCH literal.
#[cfg(any(target_os = "linux", test))]
pub(super) fn tokenize_search_literal(
    line: &str,
    literal: &str,
) -> Result<Vec<Token>, &'static str> {
    tokenize_internal(line, Some(literal))
}

/// Объединяет обычную wire-грамматику с одним внутренним, уже проверенным literal.
fn tokenize_internal(line: &str, mut literal: Option<&str>) -> Result<Vec<Token>, &'static str> {
    const MAX_TOKENS: usize = 64;
    let mut chars = line.trim_end_matches(['\r', '\n']).chars().peekable();
    let mut tokens = Vec::new();
    while let Some(character) = chars.next() {
        match character {
            value if value.is_ascii_whitespace() => {}
            '\0' => match literal.take() {
                Some(value) => push(&mut tokens, Token::Quoted(value.to_owned()), MAX_TOKENS)?,
                None => return Err("unexpected NUL in IMAP command"),
            },
            '(' => push(&mut tokens, Token::ListStart, MAX_TOKENS)?,
            ')' => push(&mut tokens, Token::ListEnd, MAX_TOKENS)?,
            '"' => {
                let value = quoted(&mut chars)?;
                push(&mut tokens, Token::Quoted(value), MAX_TOKENS)?;
            }
            first => {
                let value = atom(first, &mut chars);
                push(&mut tokens, Token::Text(value), MAX_TOKENS)?;
            }
        }
    }
    if literal.is_some() {
        return Err("SEARCH literal placeholder is missing");
    }
    Ok(tokens)
}

fn push(tokens: &mut Vec<Token>, token: Token, limit: usize) -> Result<(), &'static str> {
    if tokens.len() >= limit {
        return Err("too many IMAP tokens");
    }
    tokens.push(token);
    Ok(())
}

fn atom<I>(first: char, chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut value = String::from(first);
    while let Some(next) = chars.peek().copied() {
        match next {
            value if value.is_ascii_whitespace() => break,
            '(' | ')' => break,
            character => {
                value.push(character);
                chars.next();
            }
        }
    }
    value
}

fn quoted<I>(chars: &mut I) -> Result<String, &'static str>
where
    I: Iterator<Item = char>,
{
    let mut value = String::new();
    let mut escaped = false;
    for character in chars.by_ref() {
        match (escaped, character) {
            (true, escaped_character @ ('"' | '\\')) => {
                value.push(escaped_character);
                escaped = false;
            }
            (true, _) => return Err("invalid quoted-pair in IMAP quoted string"),
            (false, '\\') => escaped = true,
            (false, '"') => return Ok(value),
            (false, ordinary) if ordinary.is_ascii() => value.push(ordinary),
            (false, _) => return Err("non-ASCII character in IMAP4rev1 quoted string"),
        }
    }
    Err("unterminated IMAP quoted string")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Лексер сохраняет синтаксический вид quoted string и декодирует его escape.
    #[test]
    fn quoted_string_is_one_token() {
        assert_eq!(
            tokenize("a LOGIN \"bob smith\" \"p\\\"ass\"\r\n"),
            Ok(vec![
                Token::Text("a".into()),
                Token::Text("LOGIN".into()),
                Token::Quoted("bob smith".into()),
                Token::Quoted("p\"ass".into()),
            ])
        );
    }

    /// Скобки списка являются отдельными токенами даже без пробелов.
    #[test]
    fn list_boundaries_are_tokens() {
        assert_eq!(
            tokenize("a FETCH 1 (BODYSTRUCTURE)\r\n"),
            Ok(vec![
                Token::Text("a".into()),
                Token::Text("FETCH".into()),
                Token::Text("1".into()),
                Token::ListStart,
                Token::Text("BODYSTRUCTURE".into()),
                Token::ListEnd,
            ])
        );
    }

    /// Quoted string остаётся отличим от atom для проверки грамматики команд.
    #[test]
    fn quoted_and_atom_tokens_keep_distinct_variants() {
        assert_eq!(
            tokenize("a ENABLE X-TEST \"Y-TEST\"\r\n"),
            Ok(vec![
                Token::Text("a".into()),
                Token::Text("ENABLE".into()),
                Token::Text("X-TEST".into()),
                Token::Quoted("Y-TEST".into()),
            ])
        );
    }

    /// RFC 3501 разрешает quoted-pair только для кавычки и обратной косой черты.
    #[test]
    fn quoted_pair_rejects_other_escaped_characters() {
        assert!(tokenize("a LOGIN \"bob\\q\" \"pass\"\r\n").is_err());
        assert!(tokenize("a LOGIN \"bob\\ \" \"pass\"\r\n").is_err());
    }

    /// IMAP4rev1 quoted-string не допускает UTF-8 без отдельного согласования.
    #[test]
    fn wire_quoted_string_rejects_high_bit_but_literal_token_keeps_utf8() {
        assert!(tokenize("a LOGIN \"bøb\" \"pass\"\r\n").is_err());
        assert_eq!(
            tokenize_search_literal("a SEARCH SUBJECT \0\r\n", "bøb"),
            Ok(vec![
                Token::Text("a".into()),
                Token::Text("SEARCH".into()),
                Token::Text("SUBJECT".into()),
                Token::Quoted("bøb".into()),
            ])
        );
    }
}
