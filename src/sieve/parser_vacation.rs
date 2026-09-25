//! Разбор параметров RFC 5230 vacation action до typed AST.

use super::super::super::ast::{Action, VacationAction};
use super::super::super::lexer::Token;
use super::super::super::parser_values::{atom, one_string, string_list};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Parameter {
    Days,
    Subject,
    From,
    Addresses,
    Mime,
    Handle,
}

/// Преобразует wire tags в Enum до проверки повторов и значений.
fn parameter(value: &str) -> Option<Parameter> {
    match value {
        ":days" => Some(Parameter::Days),
        ":subject" => Some(Parameter::Subject),
        ":from" => Some(Parameter::From),
        ":addresses" => Some(Parameter::Addresses),
        ":mime" => Some(Parameter::Mime),
        ":handle" => Some(Parameter::Handle),
        _ => None,
    }
}

pub(super) fn parse(input: &mut VecDeque<Token>) -> Result<Action, &'static str> {
    let mut seen = HashSet::new();
    let mut action = VacationAction {
        days: None,
        subject: None,
        from: None,
        addresses: Vec::new(),
        mime: false,
        handle: None,
        reason: String::new(),
    };
    while let Some(Token::Atom(tag)) = input.front() {
        let Some(selected) = parameter(tag) else {
            break;
        };
        let _ = input.pop_front();
        if !seen.insert(selected) {
            return Err("duplicate vacation parameter");
        }
        match selected {
            Parameter::Days => {
                action.days = Some(atom(input)?.parse().map_err(|_| "invalid vacation days")?)
            }
            Parameter::Subject => action.subject = Some(one_string(input)?),
            Parameter::From => action.from = Some(one_string(input)?),
            Parameter::Addresses => action.addresses = string_list(input)?,
            Parameter::Mime => action.mime = true,
            Parameter::Handle => action.handle = Some(one_string(input)?),
        }
    }
    action.reason = one_string(input)?;
    Ok(Action::Vacation(action))
}
