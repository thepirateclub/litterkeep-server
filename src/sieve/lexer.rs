//! Лексер Sieve с комментариями и строками, не использующий regex.

use std::iter::Peekable;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Token {
    Atom(String),
    /// RFC 5228 strings остаются октетами до выбора их текстовой семантики.
    String(Vec<u8>),
    OpenList,
    CloseList,
    OpenTestList,
    CloseTestList,
    OpenBlock,
    CloseBlock,
    Semicolon,
    Comma,
}

/// Лексически разбирает script без UTF-8-преобразования строковых октетов.
pub(super) fn tokenize_octets(source: &[u8]) -> Result<Vec<Token>, &'static str> {
    validate_source_octets(source)?;
    let mut characters = source.iter().copied().peekable();
    let mut tokens = Vec::new();
    while let Some(character) = characters.next() {
        match character {
            b' ' | b'\t' => {}
            b'\r' if characters.next() == Some(b'\n') => {}
            b'\r' | b'\n' => return Err("invalid sieve line ending"),
            b'#' => line_comment(&mut characters)?,
            b'/' => block_comment(&mut characters)?,
            b'"' => tokens.push(Token::String(string(&mut characters)?)),
            b'[' => tokens.push(Token::OpenList),
            b']' => tokens.push(Token::CloseList),
            b'(' => tokens.push(Token::OpenTestList),
            b')' => tokens.push(Token::CloseTestList),
            b'{' => tokens.push(Token::OpenBlock),
            b'}' => tokens.push(Token::CloseBlock),
            b';' => tokens.push(Token::Semicolon),
            b',' => tokens.push(Token::Comma),
            value => {
                let word = atom(value, &mut characters);
                let word = match String::from_utf8(word) {
                    Ok(word) if word.is_ascii() => word,
                    _ => return Err("non-ASCII sieve syntax token"),
                };
                match word.eq_ignore_ascii_case("text:") {
                    true => tokens.push(Token::String(multiline(&mut characters)?)),
                    false => tokens.push(Token::Atom(word)),
                }
            }
        }
        if tokens.len() > 512 {
            return Err("sieve token limit exceeded");
        }
    }
    Ok(tokens)
}

/// Запрещает NUL и непарные CR/LF во всех областях, включая комментарии.
fn validate_source_octets(source: &[u8]) -> Result<(), &'static str> {
    let mut characters = source.iter().copied().peekable();
    while let Some(character) = characters.next() {
        match character {
            0 => return Err("NUL is not permitted in sieve scripts"),
            b'\r' if characters.next() == Some(b'\n') => {}
            b'\r' | b'\n' => return Err("invalid sieve line ending"),
            _ => {}
        }
    }
    Ok(())
}

/// Hash-comment обязана завершаться CRLF; EOF не завершает комментарий.
fn line_comment(characters: &mut impl Iterator<Item = u8>) -> Result<(), &'static str> {
    while let Some(character) = characters.next() {
        if character == b'\r' {
            let _ = characters.next();
            return Ok(());
        }
    }
    Err("unterminated sieve hash comment")
}

fn block_comment(characters: &mut impl Iterator<Item = u8>) -> Result<(), &'static str> {
    if characters.next() != Some(b'*') {
        return Err("unexpected slash in sieve");
    }
    let mut star = false;
    for character in characters.by_ref() {
        if matches!((star, character), (true, b'/')) {
            return Ok(());
        }
        star = character == b'*';
    }
    Err("unterminated sieve comment")
}

/// Читает RFC 5228 quoted-string, сохраняя CRLF и допустимые control octets.
fn string(characters: &mut impl Iterator<Item = u8>) -> Result<Vec<u8>, &'static str> {
    let mut result = Vec::new();
    while let Some(character) = characters.next() {
        match character {
            b'"' => return Ok(result),
            b'\\' => match characters.next() {
                Some(value @ (b'"' | b'\\')) => result.push(value),
                Some(value) if !matches!(value, 0 | b'\r' | b'\n') => result.push(value),
                _ => return Err("invalid sieve string escape"),
            },
            b'\r' if characters.next() == Some(b'\n') => result.extend_from_slice(b"\r\n"),
            0 | b'\r' | b'\n' => return Err("invalid character in sieve string"),
            value => result.push(value),
        }
    }
    Err("unterminated sieve string")
}

/// Читает RFC 5228 `text:` строку с CRLF-разделителями и dot-unstuffing.
fn multiline(characters: &mut Peekable<impl Iterator<Item = u8>>) -> Result<Vec<u8>, &'static str> {
    while matches!(characters.peek(), Some(b' ' | b'\t')) {
        let _ = characters.next();
    }
    match characters.peek().copied() {
        Some(b'#') => {
            let _ = characters.next();
            line_comment(characters)?;
        }
        Some(b'\r') => match (characters.next(), characters.next()) {
            (Some(b'\r'), Some(b'\n')) => {}
            _ => return Err("invalid sieve multiline header"),
        },
        _ => return Err("invalid sieve multiline header"),
    }
    multiline_body(characters)
}

/// Снимает ровно один dot-stuffing octet и сохраняет CRLF перед terminator.
fn multiline_body(
    characters: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<Vec<u8>, &'static str> {
    let mut result = Vec::new();
    loop {
        let line = multiline_line(characters)?;
        match line.as_slice() {
            b"." => return Ok(result),
            _ => {
                let unstuffed = match line.strip_prefix(b"..") {
                    Some(_) => line.strip_prefix(b".").unwrap_or(line.as_slice()),
                    None => line.as_slice(),
                };
                result.extend_from_slice(unstuffed);
                result.extend_from_slice(b"\r\n");
                if result.len() > 65_536 {
                    return Err("sieve multiline string limit exceeded");
                }
            }
        }
    }
}

/// Читает одну CRLF-завершённую строку тела, не включая её перевод строки.
fn multiline_line(
    characters: &mut Peekable<impl Iterator<Item = u8>>,
) -> Result<Vec<u8>, &'static str> {
    let mut line = Vec::new();
    loop {
        match characters.next() {
            Some(b'\r') => {
                let _ = characters.next();
                return Ok(line);
            }
            Some(character) => line.push(character),
            None => return Err("unterminated sieve multiline string"),
        }
    }
}

fn atom(first: u8, characters: &mut Peekable<impl Iterator<Item = u8>>) -> Vec<u8> {
    let mut value = vec![first];
    while let Some(next) = characters.peek().copied() {
        if boundary(next) {
            break;
        }
        let _ = characters.next();
        value.push(next);
    }
    value
}

/// Отделяет атом от структурного token-а без составной логической цепочки.
fn boundary(character: u8) -> bool {
    match character.is_ascii_whitespace() {
        true => true,
        false => matches!(
            character,
            b'#' | b'/' | b'"' | b'[' | b']' | b'(' | b')' | b'{' | b'}' | b';' | b','
        ),
    }
}

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod tests;
