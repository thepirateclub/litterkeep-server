//! Разбор атомов и строковых форм Sieve, общий для parser grammar.
//!
//! Этот файл не выбирает команду, test или action. Его единственная
//! ответственность, извлечь ограниченные lexical values из входной очереди.

use super::lexer::Token;
use std::collections::VecDeque;

/// Читает список строк, после которого grammar требует semicolon.
pub(super) fn terminated_strings(input: &mut VecDeque<Token>) -> Result<Vec<String>, &'static str> {
    let values = string_list(input)?;
    expect(input, Token::Semicolon)?;
    Ok(values)
}

/// Читает одну quoted string либо RFC-style bracket list строк.
pub(super) fn string_list(input: &mut VecDeque<Token>) -> Result<Vec<String>, &'static str> {
    match input.front() {
        Some(Token::OpenList) => {
            let _ = input.pop_front();
            let mut values = vec![one_string(input)?];
            loop {
                match input.front() {
                    Some(Token::Comma) => {
                        let _ = input.pop_front();
                        values.push(one_string(input)?);
                    }
                    Some(Token::CloseList) => {
                        let _ = input.pop_front();
                        return Ok(values);
                    }
                    _ => return Err("invalid sieve string list separator"),
                }
            }
        }
        _ => one_string(input).map(|value| vec![value]),
    }
}

/// Читает ровно одну quoted string, не принимая atom как неявную строку.
pub(super) fn one_string(input: &mut VecDeque<Token>) -> Result<String, &'static str> {
    match input.pop_front() {
        Some(Token::String(value)) => String::from_utf8(value)
            .map_err(|_| "non-UTF-8 sieve string is not supported by the text AST"),
        _ => Err("sieve string expected"),
    }
}

/// Читает один atom для keyword или numeric size argument.
pub(super) fn atom(input: &mut VecDeque<Token>) -> Result<String, &'static str> {
    match input.pop_front() {
        Some(Token::Atom(value)) => Ok(value),
        _ => Err("sieve atom expected"),
    }
}

/// Требует конкретный structural token без текстового сравнения.
pub(super) fn expect(input: &mut VecDeque<Token>, expected: Token) -> Result<(), &'static str> {
    match input.pop_front() {
        Some(actual) if actual == expected => Ok(()),
        _ => Err("unexpected sieve token"),
    }
}
