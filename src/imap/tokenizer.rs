//! Лексический анализ одной завершённой IMAP-командной строки.
//!
//! Лексер проходит вход один раз и не копирует его многократно. Кавычки и
//! экранирование обрабатываются здесь, поэтому предметный parser работает с
//! готовыми токенами и не восстанавливает синтаксис из строковых эвристик.

/// Синтаксический токен IMAP-команды.
#[derive(Debug, Eq, PartialEq)]
pub enum Token {
    /// Atom либо уже декодированная quoted string.
    Text(String),
    /// Открывающая круглая скобка списка.
    ListStart,
    /// Закрывающая круглая скобка списка.
    ListEnd,
}

/// Разбирает одну CRLF-строку за `O(n)` времени с ограничением числа токенов.
pub fn tokenize(line: &str) -> Result<Vec<Token>, &'static str> {
    const MAX_TOKENS: usize = 64;
    let mut chars = line.trim_end_matches(['\r', '\n']).chars().peekable();
    let mut tokens = Vec::new();
    while let Some(character) = chars.next() {
        match character {
            value if value.is_ascii_whitespace() => {}
            '(' => push(&mut tokens, Token::ListStart, MAX_TOKENS)?,
            ')' => push(&mut tokens, Token::ListEnd, MAX_TOKENS)?,
            '"' => {
                let value = quoted(&mut chars)?;
                push(&mut tokens, Token::Text(value), MAX_TOKENS)?;
            }
            first => {
                let value = atom(first, &mut chars);
                push(&mut tokens, Token::Text(value), MAX_TOKENS)?;
            }
        }
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
            (true, escaped_character) => {
                value.push(escaped_character);
                escaped = false;
            }
            (false, '\\') => escaped = true,
            (false, '"') => return Ok(value),
            (false, ordinary) => value.push(ordinary),
        }
    }
    Err("unterminated IMAP quoted string")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Лексер сохраняет пробелы и декодирует escape только внутри quoted string.
    #[test]
    fn quoted_string_is_one_token() {
        assert_eq!(
            tokenize("a LOGIN \"bob smith\" \"p\\\"ass\"\r\n"),
            Ok(vec![
                Token::Text("a".into()),
                Token::Text("LOGIN".into()),
                Token::Text("bob smith".into()),
                Token::Text("p\"ass".into()),
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
}
