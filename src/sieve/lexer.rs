//! Лексер Sieve с комментариями и строками, не использующий regex.

use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Token {
    Atom(String),
    String(String),
    OpenList,
    CloseList,
    OpenTestList,
    CloseTestList,
    OpenBlock,
    CloseBlock,
    Semicolon,
    Comma,
}

pub(super) fn tokenize(source: &str) -> Result<Vec<Token>, &'static str> {
    validate_source_octets(source)?;
    let mut characters = source.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(character) = characters.next() {
        match character {
            ' ' | '\t' => {}
            '\r' if characters.next() == Some('\n') => {}
            '\r' | '\n' => return Err("invalid sieve line ending"),
            '#' => line_comment(&mut characters)?,
            '/' => block_comment(&mut characters)?,
            '"' => tokens.push(Token::String(string(&mut characters)?)),
            '[' => tokens.push(Token::OpenList),
            ']' => tokens.push(Token::CloseList),
            '(' => tokens.push(Token::OpenTestList),
            ')' => tokens.push(Token::CloseTestList),
            '{' => tokens.push(Token::OpenBlock),
            '}' => tokens.push(Token::CloseBlock),
            ';' => tokens.push(Token::Semicolon),
            ',' => tokens.push(Token::Comma),
            value => {
                let word = atom(value, &mut characters);
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
fn validate_source_octets(source: &str) -> Result<(), &'static str> {
    let mut characters = source.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\0' => return Err("NUL is not permitted in sieve scripts"),
            '\r' if characters.next() == Some('\n') => {}
            '\r' | '\n' => return Err("invalid sieve line ending"),
            _ => {}
        }
    }
    Ok(())
}

/// Hash-comment обязана завершаться CRLF; EOF не завершает комментарий.
fn line_comment(characters: &mut Peekable<Chars<'_>>) -> Result<(), &'static str> {
    while let Some(character) = characters.next() {
        if character == '\r' {
            let _ = characters.next();
            return Ok(());
        }
    }
    Err("unterminated sieve hash comment")
}

fn block_comment(characters: &mut Peekable<Chars<'_>>) -> Result<(), &'static str> {
    if characters.next() != Some('*') {
        return Err("unexpected slash in sieve");
    }
    let mut star = false;
    for character in characters.by_ref() {
        if matches!((star, character), (true, '/')) {
            return Ok(());
        }
        star = character == '*';
    }
    Err("unterminated sieve comment")
}

/// Читает RFC 5228 quoted-string, сохраняя CRLF и допустимые control octets.
fn string(characters: &mut Peekable<Chars<'_>>) -> Result<String, &'static str> {
    let mut result = String::new();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Ok(result),
            '\\' => match characters.next() {
                Some(value @ ('"' | '\\')) => result.push(value),
                Some(value) if !matches!(value, '\0' | '\r' | '\n') => result.push(value),
                _ => return Err("invalid sieve string escape"),
            },
            '\r' if characters.next() == Some('\n') => result.push_str("\r\n"),
            '\0' | '\r' | '\n' => return Err("invalid character in sieve string"),
            value => result.push(value),
        }
    }
    Err("unterminated sieve string")
}

/// Читает RFC 5228 `text:` строку с CRLF-разделителями и dot-unstuffing.
fn multiline(characters: &mut Peekable<Chars<'_>>) -> Result<String, &'static str> {
    while matches!(characters.peek(), Some(' ' | '\t')) {
        let _ = characters.next();
    }
    match characters.peek().copied() {
        Some('#') => {
            let _ = characters.next();
            line_comment(characters)?;
        }
        Some('\r') => match (characters.next(), characters.next()) {
            (Some('\r'), Some('\n')) => {}
            _ => return Err("invalid sieve multiline header"),
        },
        _ => return Err("invalid sieve multiline header"),
    }
    multiline_body(characters)
}

/// Снимает ровно один dot-stuffing octet и сохраняет CRLF перед terminator.
fn multiline_body(characters: &mut Peekable<Chars<'_>>) -> Result<String, &'static str> {
    let mut result = String::new();
    loop {
        let line = multiline_line(characters)?;
        match line.as_str() {
            "." => return Ok(result),
            _ => {
                let unstuffed = match line.strip_prefix("..") {
                    Some(_) => line.strip_prefix('.').unwrap_or(line.as_str()),
                    None => line.as_str(),
                };
                result.push_str(unstuffed);
                result.push_str("\r\n");
                if result.len() > 65_536 {
                    return Err("sieve multiline string limit exceeded");
                }
            }
        }
    }
}

/// Читает одну CRLF-завершённую строку тела, не включая её перевод строки.
fn multiline_line(characters: &mut Peekable<Chars<'_>>) -> Result<String, &'static str> {
    let mut line = String::new();
    loop {
        match characters.next() {
            Some('\r') => {
                let _ = characters.next();
                return Ok(line);
            }
            Some(character) => line.push(character),
            None => return Err("unterminated sieve multiline string"),
        }
    }
}

fn atom(first: char, characters: &mut Peekable<Chars<'_>>) -> String {
    let mut value = String::from(first);
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
fn boundary(character: char) -> bool {
    match character.is_ascii_whitespace() {
        true => true,
        false => matches!(
            character,
            '#' | '/' | '"' | '[' | ']' | '(' | ')' | '{' | '}' | ';' | ','
        ),
    }
}

#[cfg(test)]
#[path = "lexer_tests.rs"]
mod tests;
