//! Рекурсивный parser начального совместимого подмножества Sieve.

use super::ast::{Action, Command, Condition, Script};
use super::lexer::Token;
use std::collections::VecDeque;

pub(super) fn parse(tokens: Vec<Token>) -> Result<Script, &'static str> {
    let mut input: VecDeque<Token> = tokens.into();
    let mut requires = Vec::new();
    let mut commands = Vec::new();
    while let Some(token) = input.pop_front() {
        let Token::Atom(word) = token else {
            return Err("sieve command expected");
        };
        match word.as_str() {
            "require" => requires.extend(strings(&mut input)?),
            "if" => commands.push(if_command(&mut input)?),
            word => action(word, &mut input, &mut commands)?,
        }
    }
    Ok(Script { requires, commands })
}

fn if_command(input: &mut VecDeque<Token>) -> Result<Command, &'static str> {
    let condition = condition(input)?;
    let then_commands = block(input)?;
    let else_commands = match input.front() {
        Some(Token::Atom(word)) if word == "else" => {
            let _ = input.pop_front();
            block(input)?
        }
        _ => Vec::new(),
    };
    Ok(Command::If {
        condition,
        then_commands,
        else_commands,
    })
}

fn block(input: &mut VecDeque<Token>) -> Result<Vec<Command>, &'static str> {
    expect(input, Token::OpenBlock)?;
    let mut commands = Vec::new();
    loop {
        match input.pop_front() {
            Some(Token::CloseBlock) => return Ok(commands),
            Some(Token::Atom(word)) => action(&word, input, &mut commands)?,
            _ => return Err("invalid sieve block"),
        }
    }
}

fn action(
    word: &str,
    input: &mut VecDeque<Token>,
    output: &mut Vec<Command>,
) -> Result<(), &'static str> {
    let action = match word {
        "keep" => Action::Keep,
        "discard" => Action::Discard,
        "stop" => Action::Stop,
        "fileinto" => Action::FileInto(one_string(input)?),
        "redirect" => Action::Redirect(one_string(input)?),
        "if" => {
            output.push(if_command(input)?);
            return Ok(());
        }
        _ => return Err("unsupported sieve command"),
    };
    expect(input, Token::Semicolon)?;
    output.push(Command::Action(action));
    Ok(())
}

fn condition(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let word = atom(input)?;
    match word.as_str() {
        "true" => Ok(Condition::True),
        "false" => Ok(Condition::False),
        "header" => header(input),
        "exists" => Ok(Condition::Exists {
            names: strings(input)?,
        }),
        "not" => Ok(Condition::Not(Box::new(condition(input)?))),
        "allof" => conditions(input, Condition::AllOf),
        "anyof" => conditions(input, Condition::AnyOf),
        _ => Err("unsupported sieve test"),
    }
}

fn header(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    match input.pop_front() {
        Some(Token::Atom(tag)) if tag == ":contains" => {}
        _ => return Err("only :contains header matcher is supported"),
    }
    Ok(Condition::HeaderContains {
        name: one_string(input)?,
        needle: one_string(input)?,
    })
}

fn conditions(
    input: &mut VecDeque<Token>,
    make: fn(Vec<Condition>) -> Condition,
) -> Result<Condition, &'static str> {
    expect(input, Token::OpenList)?;
    let mut values = Vec::new();
    loop {
        match input.front() {
            Some(Token::CloseList) => {
                let _ = input.pop_front();
                return Ok(make(values));
            }
            Some(Token::Comma) => {
                let _ = input.pop_front();
            }
            Some(_) => values.push(condition(input)?),
            None => return Err("unterminated sieve condition list"),
        }
    }
}

fn strings(input: &mut VecDeque<Token>) -> Result<Vec<String>, &'static str> {
    match input.front() {
        Some(Token::OpenList) => {
            let _ = input.pop_front();
            let mut values = Vec::new();
            loop {
                match input.pop_front() {
                    Some(Token::String(value)) => values.push(value),
                    Some(Token::Comma) => {}
                    Some(Token::CloseList) => {
                        expect(input, Token::Semicolon)?;
                        return Ok(values);
                    }
                    _ => return Err("invalid sieve string list"),
                }
            }
        }
        _ => {
            let value = one_string(input)?;
            expect(input, Token::Semicolon)?;
            Ok(vec![value])
        }
    }
}

fn one_string(input: &mut VecDeque<Token>) -> Result<String, &'static str> {
    match input.pop_front() {
        Some(Token::String(value)) => Ok(value),
        _ => Err("sieve string expected"),
    }
}
fn atom(input: &mut VecDeque<Token>) -> Result<String, &'static str> {
    match input.pop_front() {
        Some(Token::Atom(value)) => Ok(value),
        _ => Err("sieve atom expected"),
    }
}
fn expect(input: &mut VecDeque<Token>, expected: Token) -> Result<(), &'static str> {
    match input.pop_front() {
        Some(actual) if actual == expected => Ok(()),
        _ => Err("unexpected sieve token"),
    }
}
