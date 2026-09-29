//! Обходит string arguments AST для compile-time Sieve namespace checks.

use super::super::ast::{Action, Condition};

/// Проверяет все динамические strings у одного action.
pub(super) fn action(action: &Action) -> Result<(), &'static str> {
    match action {
        Action::FileInto(value)
        | Action::FileIntoCopy(value)
        | Action::Redirect(value)
        | Action::RedirectCopy(value) => validate(value),
        Action::Set(action) => validate(&action.value),
        Action::Vacation(action) => {
            validate(&action.reason)?;
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

/// Проверяет строки теста и рекурсивно обходит composed conditions.
pub(super) fn condition(value: &Condition) -> Result<(), &'static str> {
    match value {
        Condition::Header { names, matcher } | Condition::Address { names, matcher, .. } => {
            strings(names)?;
            strings(&matcher.keys)?;
            comparator(&matcher.comparator)
        }
        Condition::Envelope { matcher, .. } => {
            strings(&matcher.keys)?;
            comparator(&matcher.comparator)
        }
        Condition::String { sources, matcher } => {
            strings(sources)?;
            strings(&matcher.keys)?;
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
