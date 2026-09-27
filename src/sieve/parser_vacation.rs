//! Разбор параметров RFC 5230 vacation action до typed AST.

use super::super::super::ast::{Action, VacationAction, VacationInterval};
use super::super::super::lexer::Token;
use super::super::super::parser_values::{atom, one_string, string_list};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Parameter {
    Days,
    Seconds,
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
        ":seconds" => Some(Parameter::Seconds),
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
        interval: None,
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
            Parameter::Days | Parameter::Seconds if action.interval.is_some() => {
                return Err("vacation days and seconds are mutually exclusive");
            }
            Parameter::Days => {
                action.interval = Some(parse_interval(Parameter::Days, input)?);
            }
            Parameter::Seconds => {
                action.interval = Some(parse_interval(Parameter::Seconds, input)?);
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

/// Разбирает обе RFC-формы в одном поле, исключая их совместное применение.
fn parse_interval(
    parameter: Parameter,
    input: &mut VecDeque<Token>,
) -> Result<VacationInterval, &'static str> {
    let value = atom(input)?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid vacation interval");
    }
    let value = value
        .parse::<u64>()
        .map_err(|_| "invalid vacation interval")?;
    match parameter {
        Parameter::Days => Ok(VacationInterval::Days(value)),
        Parameter::Seconds if value < (1_u64 << 31) => Ok(VacationInterval::Seconds(value)),
        Parameter::Seconds => Err("vacation seconds out of range"),
        _ => Err("invalid vacation interval"),
    }
}
