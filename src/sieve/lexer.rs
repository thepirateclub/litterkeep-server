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
            value => tokens.push(Token::Atom(atom(value, &mut characters))),
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
