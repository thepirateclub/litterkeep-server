//! Рекурсивный parser начального совместимого подмножества Sieve.

use super::ast::{
    Action, AddressPart, Capability, Command, Comparator, Condition, MatchType, Script,
    SizeRelation, StringMatch,
};
use super::lexer::Token;
use super::parser_values::{atom, expect, one_string, string_list, terminated_strings};
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
            "require" => {
                // RFC 5228 разрешает require только до первого действия или if.
                if !commands.is_empty() {
                    return Err("sieve require must precede commands");
                }
                requires.extend(terminated_strings(&mut input)?.into_iter().map(capability))
            }
            "if" => commands.push(if_command(&mut input)?),
            word => action(word, &mut input, &mut commands)?,
        }
    }
    Ok(Script { requires, commands })
}

fn capability(value: String) -> Capability {
    match value.as_str() {
        "fileinto" => Capability::FileInto,
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
        Some(Token::Atom(word)) if word == "else" => {
            let _ = input.pop_front();
            block(input)
        }
        Some(Token::Atom(word)) if word == "elsif" => {
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
        "address" => address(input),
        "exists" => Ok(Condition::Exists {
            names: string_list(input)?,
        }),
        "size" => size(input),
        "not" => Ok(Condition::Not(Box::new(condition(input)?))),
        "allof" => conditions(input, Condition::AllOf),
        "anyof" => conditions(input, Condition::AnyOf),
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
    let part = address_part(input);
    let (match_type, comparator) = match_options(input)?;
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

fn address_part(input: &mut VecDeque<Token>) -> AddressPart {
    let Some(Token::Atom(tag)) = input.front() else {
        return AddressPart::All;
    };
    let part = match tag.as_str() {
        ":all" => AddressPart::All,
        ":localpart" => AddressPart::LocalPart,
        ":domain" => AddressPart::Domain,
        ":user" => AddressPart::User,
        ":detail" => AddressPart::Detail,
        _ => return AddressPart::All,
    };
    let _ = input.pop_front();
    part
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
        match tag.as_str() {
            ":is" if !has_kind => {
                let _ = input.pop_front();
                kind = MatchType::Is;
                has_kind = true;
            }
            ":contains" if !has_kind => {
                let _ = input.pop_front();
                kind = MatchType::Contains;
                has_kind = true;
            }
            ":matches" if !has_kind => {
                let _ = input.pop_front();
                kind = MatchType::Matches;
                has_kind = true;
            }
            ":comparator" if !has_comparator => {
                let _ = input.pop_front();
                selected_comparator = parse_comparator(&one_string(input)?)?;
                has_comparator = true;
            }
            value if value.starts_with(':') => return Err("unsupported sieve match tag"),
            _ => return Ok((kind, selected_comparator)),
        }
    }
}

fn parse_comparator(value: &str) -> Result<Comparator, &'static str> {
    match value {
        "i;ascii-casemap" => Ok(Comparator::AsciiCasemap),
        "i;octet" => Ok(Comparator::Octet),
        _ => Err("unsupported sieve comparator"),
    }
}

fn size(input: &mut VecDeque<Token>) -> Result<Condition, &'static str> {
    let relation = match atom(input)?.as_str() {
        ":over" => SizeRelation::Over,
        ":under" => SizeRelation::Under,
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
