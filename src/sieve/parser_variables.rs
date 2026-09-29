//! Разбор RFC 5229 `set` action отдельно от delivery command grammar.

use super::super::super::ast::{Action, SetAction, VariableModifier};
use super::super::super::keywords::VariableModifierTag;
use super::super::super::lexer::Token;
use super::super::super::parser_values::one_string;
use super::super::super::variables::{VariableError, VariableStore};
use std::collections::VecDeque;

/// Разбирает теги до двух positional strings: constant name и expanded value.
pub(super) fn parse_set(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    let modifiers = modifiers(input)?;
    let name = one_string(input)?;
    VariableStore::validate_name(&name).map_err(invalid_name)?;
    VariableStore::validate_modifiers(&modifiers).map_err(invalid_modifiers)?;
    let value = one_string(input)?;
    Ok(Action::Set(SetAction {
        name,
        value,
        modifiers,
    }))
}

/// Преобразует case-insensitive wire tags в enum-ы без positional values.
fn modifiers(input: &mut VecDeque<Token>) -> Result<Vec<VariableModifier>, &'static str> {
    let mut result = Vec::new();
    loop {
        let Some(Token::Atom(tag)) = input.front() else {
            return Ok(result);
        };
        match VariableModifierTag::parse(tag) {
            VariableModifierTag::Lower => result.push(take(input, VariableModifier::Lower)?),
            VariableModifierTag::Upper => result.push(take(input, VariableModifier::Upper)?),
            VariableModifierTag::LowerFirst => {
                result.push(take(input, VariableModifier::LowerFirst)?)
            }
            VariableModifierTag::UpperFirst => {
                result.push(take(input, VariableModifier::UpperFirst)?)
            }
            VariableModifierTag::QuoteWildcard => {
                result.push(take(input, VariableModifier::QuoteWildcard)?)
            }
            VariableModifierTag::Length => result.push(take(input, VariableModifier::Length)?),
            VariableModifierTag::Unknown if tag.starts_with(':') => {
                return Err("unsupported variables modifier");
            }
            VariableModifierTag::Unknown => return Ok(result),
        }
    }
}

/// Снимает уже распознанный tagged argument из очереди.
fn take(
    input: &mut VecDeque<Token>,
    modifier: VariableModifier,
) -> Result<VariableModifier, &'static str> {
    match input.pop_front() {
        Some(Token::Atom(_)) => Ok(modifier),
        _ => Err("sieve modifier expected"),
    }
}

fn invalid_name(error: VariableError) -> &'static str {
    match error {
        VariableError::InvalidName => "invalid variables name",
        _ => "invalid variables name",
    }
}

fn invalid_modifiers(error: VariableError) -> &'static str {
    match error {
        VariableError::InvalidModifiers => "conflicting variables modifiers",
        _ => "invalid variables modifiers",
    }
}
