//! Обходит string arguments AST для compile-time Sieve namespace checks.

use super::super::ast::{Action, Condition};

/// Проверяет все динамические strings у одного action.
pub(super) fn action(action: &Action) -> Result<(), &'static str> {
    match action {
        Action::FileInto(value)
        | Action::FileIntoCopy(value)
        | Action::Redirect(value)
        | Action::RedirectCopy(value) => validate_octets(value),
        Action::Set(action) => validate(&action.value),
        Action::Vacation(action) => {
            validate_octets(&action.reason)?;
            optional(action.subject.as_deref())?;
            optional(action.from.as_deref())?;
            optional(action.handle.as_deref())?;
            action
                .addresses
                .iter()
                .try_for_each(|address| validate(address))
        }
        Action::Keep | Action::Discard | Action::Stop => Ok(()),
    }
}

/// Проверяет namespaces у action string, не требуя UTF-8 для literal octets.
fn validate_octets(value: &[u8]) -> Result<(), &'static str> {
    if value.windows(2).any(|marker| marker == b"${") {
        let text =
            std::str::from_utf8(value).map_err(|_| "non-UTF-8 variable reference is invalid")?;
        validate(text)?;
    }
    Ok(())
}

/// Проверяет строки теста и рекурсивно обходит composed conditions.
pub(super) fn condition(value: &Condition) -> Result<(), &'static str> {
    match value {
        Condition::Header { names, matcher } | Condition::Address { names, matcher, .. } => {
            strings(names)?;
            octets(&matcher.keys)?;
            comparator(&matcher.comparator)
        }
        Condition::Envelope { parts, matcher, .. } => {
            parts.iter().try_for_each(|part| match part {
                super::super::ast::EnvelopePartSelector::Variable(source) => validate(source),
                super::super::ast::EnvelopePartSelector::Static(_) => Ok(()),
            })?;
            octets(&matcher.keys)?;
            comparator(&matcher.comparator)
        }
        Condition::String { sources, matcher } => {
            octets(sources)?;
            octets(&matcher.keys)?;
            comparator(&matcher.comparator)
        }
        Condition::Exists { names } => strings(names),
        Condition::Not(nested) => condition(nested),
        Condition::AllOf(values) | Condition::AnyOf(values) => {
            values.iter().try_for_each(condition)
        }
        Condition::True | Condition::False | Condition::Size { .. } => Ok(()),
    }
}

fn strings(values: &[String]) -> Result<(), &'static str> {
    values.iter().try_for_each(|value| validate(value))
}

/// До byte-oriented Variables expansion отказывает на бинарных аргументах.
fn octets(values: &[Vec<u8>]) -> Result<(), &'static str> {
    for value in values {
        if value.windows(2).any(|marker| marker == b"${") {
            let text = std::str::from_utf8(value)
                .map_err(|_| "non-UTF-8 variable reference is invalid")?;
            validate(text)?;
        }
    }
    Ok(())
}

fn optional(value: Option<&str>) -> Result<(), &'static str> {
    match value {
        Some(value) => validate(value),
        None => Ok(()),
    }
}

fn comparator(value: &super::super::ast::Comparator) -> Result<(), &'static str> {
    match value {
        super::super::ast::Comparator::Variable(source) => validate(source),
        super::super::ast::Comparator::AsciiCasemap | super::super::ast::Comparator::Octet => {
            Ok(())
        }
    }
}

fn validate(value: &str) -> Result<(), &'static str> {
    super::super::variables::VariableStore::validate_namespace_references(value)
        .map_err(|_| "sieve variable namespace is not required")
}
