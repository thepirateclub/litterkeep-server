//! Рекурсивный parser начального совместимого подмножества Sieve.

use super::address_options;
use super::ast::{
    Action, Capability, Command, Comparator, Condition, EnvelopePart, MatchType, Script,
    SizeRelation, StringMatch,
};
use super::keywords::{CapabilityWord, CommandWord, ComparatorWord, MatchTag, SizeTag, TestWord};
use super::lexer::Token;
use super::parser_values::{atom, expect, one_string, string_list, terminated_strings};
#[path = "parser_actions.rs"]
mod actions;
use std::collections::VecDeque;

pub(super) fn parse(tokens: Vec<Token>) -> Result<Script, &'static str> {
    let mut input: VecDeque<Token> = tokens.into();
    let mut requires = Vec::new();
    let mut commands = Vec::new();
    while let Some(token) = input.pop_front() {
        let Token::Atom(word) = token else {
            return Err("sieve command expected");
        };
        match CommandWord::parse(&word) {
            CommandWord::Require => {
                // RFC 5228 разрешает require только до первого действия или if.
                if !commands.is_empty() {
                    return Err("sieve require must precede commands");
                }
                requires.extend(terminated_strings(&mut input)?.into_iter().map(capability))
            }
            CommandWord::If => commands.push(if_command(&mut input)?),
            command => action(command, &mut input, &mut commands)?,
        }
    }
    Ok(Script { requires, commands })
}

fn capability(value: String) -> Capability {
    match CapabilityWord::parse(&value) {
        CapabilityWord::FileInto => Capability::FileInto,
        CapabilityWord::Copy => Capability::Copy,
        CapabilityWord::Subaddress => Capability::Subaddress,
        CapabilityWord::Envelope => Capability::Envelope,
        CapabilityWord::Vacation => Capability::Vacation,
        CapabilityWord::VacationSeconds => Capability::VacationSeconds,
        _ => Capability::Unsupported(value),
    }
}

fn if_command(input: &mut VecDeque<Token>) -> Result<Command, &'static str> {
    let condition = condition(input)?;
    let then_commands = block(input)?;
    let else_commands = alternative(input)?;
    Ok(Command::If {
        condition,
        then_commands,
        else_commands,
    })
}

/// Представляет `elsif` как вложенный typed If в единственной else-ветви.
///
/// Executor уже умеет рекурсивно исполнять `Command::If`, поэтому такой AST
/// не создаёт второй алгоритм выбора ветви и сохраняет общий лимит команд.
fn alternative(input: &mut VecDeque<Token>) -> Result<Vec<Command>, &'static str> {
    match input.front() {
        Some(Token::Atom(word)) if CommandWord::parse(word) == CommandWord::Else => {
            let _ = input.pop_front();
            block(input)
        }
        Some(Token::Atom(word)) if CommandWord::parse(word) == CommandWord::Elsif => {
            let _ = input.pop_front();
            Ok(vec![if_command(input)?])
        }
        _ => Ok(Vec::new()),
    }
}

fn block(input: &mut VecDeque<Token>) -> Result<Vec<Command>, &'static str> {
    expect(input, Token::OpenBlock)?;
    let mut commands = Vec::new();
    loop {
        match input.pop_front() {
            Some(Token::CloseBlock) => return Ok(commands),
            Some(Token::Atom(word)) => action(CommandWord::parse(&word), input, &mut commands)?,
            _ => return Err("invalid sieve block"),
        }
    }
}

fn action(
    word: CommandWord,
    input: &mut VecDeque<Token>,
    output: &mut Vec<Command>,
) -> Result<(), &'static str> {
    let action = match word {
        CommandWord::Keep => Action::Keep,
        CommandWord::Discard => Action::Discard,
        CommandWord::Stop => Action::Stop,
        CommandWord::FileInto => actions::fileinto(input)?,
        CommandWord::Redirect => actions::redirect(input)?,
        CommandWord::Vacation => actions::vacation(input)?,
        CommandWord::If => {
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
    match TestWord::parse(&word) {
        TestWord::True => Ok(Condition::True),
        TestWord::False => Ok(Condition::False),
        TestWord::Header => header(input),
        TestWord::Address => address(input),
        TestWord::Envelope => envelope(input),
        TestWord::Exists => Ok(Condition::Exists {
            names: string_list(input)?,
        }),
        TestWord::Size => size(input),
        TestWord::Not => Ok(Condition::Not(Box::new(condition(input)?))),
        TestWord::AllOf => conditions(input, Condition::AllOf),
        TestWord::AnyOf => conditions(input, Condition::AnyOf),
        _ => Err("unsupported sieve test"),
    }
}

fn header(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let (match_type, comparator) = match_options(input)?;
    Ok(Condition::Header {
        names: string_list(input)?,
        matcher: StringMatch {
            match_type,
            comparator,
            keys: string_list(input)?,
        },
    })
}

fn address(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let (part, match_type, comparator) = address_options::parse(input)?;
    Ok(Condition::Address {
        names: string_list(input)?,
        part,
        matcher: StringMatch {
            match_type,
            comparator,
            keys: string_list(input)?,
        },
    })
}

/// Разбирает SMTP MAIL FROM/RCPT TO selector-ы и общий match contract.
fn envelope(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let (part, match_type, comparator) = address_options::parse(input)?;
    let parts = string_list(input)?
        .into_iter()
        .map(envelope_part)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Condition::Envelope {
        parts,
        part,
        matcher: StringMatch {
            match_type,
            comparator,
            keys: string_list(input)?,
        },
    })
}

/// Переводит wire selector в enum до передачи теста executor-у.
fn envelope_part(value: String) -> Result<EnvelopePart, &'static str> {
    match (
        value.eq_ignore_ascii_case("from"),
        value.eq_ignore_ascii_case("to"),
    ) {
        (true, false) => Ok(EnvelopePart::From),
        (false, true) => Ok(EnvelopePart::To),
        _ => Err("unsupported sieve envelope part"),
    }
}

/// Разбирает разрешённые match-tags без смешивания с аргументами test-а.
fn match_options(input: &mut VecDeque<Token>) -> Result<(MatchType, Comparator), &'static str> {
    let mut kind = MatchType::Is;
    let mut selected_comparator = Comparator::AsciiCasemap;
    let mut has_kind = false;
    let mut has_comparator = false;
    loop {
        let Some(Token::Atom(tag)) = input.front() else {
            return Ok((kind, selected_comparator));
        };
        match (MatchTag::parse(tag), has_kind, has_comparator) {
            (MatchTag::Is, false, _) => {
                let _ = input.pop_front();
                kind = MatchType::Is;
                has_kind = true;
            }
            (MatchTag::Contains, false, _) => {
                let _ = input.pop_front();
                kind = MatchType::Contains;
                has_kind = true;
            }
            (MatchTag::Matches, false, _) => {
                let _ = input.pop_front();
                kind = MatchType::Matches;
                has_kind = true;
            }
            (MatchTag::Comparator, _, false) => {
                let _ = input.pop_front();
                selected_comparator = parse_comparator(&one_string(input)?)?;
                has_comparator = true;
            }
            (MatchTag::Unknown, _, _) if tag.starts_with(':') => {
                return Err("unsupported sieve match tag");
            }
            _ => return Ok((kind, selected_comparator)),
        }
    }
}

fn parse_comparator(value: &str) -> Result<Comparator, &'static str> {
    match ComparatorWord::parse(value) {
        ComparatorWord::AsciiCasemap => Ok(Comparator::AsciiCasemap),
        ComparatorWord::Octet => Ok(Comparator::Octet),
        _ => Err("unsupported sieve comparator"),
    }
}

fn size(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let relation = match SizeTag::parse(&atom(input)?) {
        SizeTag::Over => SizeRelation::Over,
        SizeTag::Under => SizeRelation::Under,
        _ => return Err("sieve size relation expected"),
    };
    Ok(Condition::Size {
        relation,
        limit: size_limit(&atom(input)?)?,
    })
}

/// RFC-стиль `K`, `M`, `G` quantifier переводится в октеты с проверкой overflow.
fn size_limit(value: &str) -> Result<usize, &'static str> {
    let (number, factor) = match value.chars().last() {
        Some('K' | 'k') => (value.get(..value.len().saturating_sub(1)), 1_024_usize),
        Some('M' | 'm') => (value.get(..value.len().saturating_sub(1)), 1_048_576_usize),
        Some('G' | 'g') => (
            value.get(..value.len().saturating_sub(1)),
            1_073_741_824_usize,
        ),
        Some(_) => (Some(value), 1),
        None => (None, 1),
    };
    number
        .ok_or("invalid sieve size")?
        .parse::<usize>()
        .map_err(|_| "invalid sieve size")?
        .checked_mul(factor)
        .ok_or("sieve size overflow")
}

fn conditions(
    input: &mut VecDeque<Token>,
    make: fn(Vec<Condition>) -> Condition,
) -> Result<Condition, &'static str> {
    expect(input, Token::OpenTestList)?;
    let mut values = Vec::new();
    values.push(condition(input)?);
    loop {
        match input.front() {
            Some(Token::CloseTestList) => {
                let _ = input.pop_front();
                return Ok(make(values));
            }
            Some(Token::Comma) => {
                let _ = input.pop_front();
                values.push(condition(input)?);
            }
            _ => return Err("invalid sieve test-list separator"),
        }
    }
}
