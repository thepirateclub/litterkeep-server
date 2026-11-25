//! Общие преобразования динамических Sieve action arguments.

use super::{ExecutionError, ExecutionState};
use crate::sieve::ast::VacationAction;

/// Раскрывает action string только в script, запросившем Variables extension.
pub(super) fn expand(value: &str, state: &ExecutionState) -> Result<String, ExecutionError> {
    if !state.variables_enabled {
        return Ok(value.to_owned());
    }
    state
        .variables
        .expand_octets(value, state.match_variables.octets())
        .map_err(|_error| ExecutionError::VariableRuntime)
}

/// Преобразует action octets в текст на границе mailbox/address runtime API.
pub(super) fn expand_octets(
    value: &[u8],
    state: &ExecutionState,
) -> Result<String, ExecutionError> {
    let text = std::str::from_utf8(value).map_err(|_| ExecutionError::VariableRuntime)?;
    expand(text, state)
}

/// Раскрывает каждое строковое поле vacation, не меняя числовые policy tags.
pub(super) fn vacation(
    action: &VacationAction,
    state: &ExecutionState,
) -> Result<VacationAction, ExecutionError> {
    Ok(VacationAction {
        interval: action.interval,
        subject: optional(action.subject.as_deref(), state)?,
        from: optional(action.from.as_deref(), state)?,
        addresses: action
            .addresses
            .iter()
            .map(|value| expand(value, state))
            .collect::<Result<Vec<_>, _>>()?,
        mime: action.mime,
        handle: optional(action.handle.as_deref(), state)?,
        reason: expand_reason(&action.reason, state)?,
    })
}

/// Сохраняет binary reason без marker-а, но раскрывает Variables только из UTF-8.
fn expand_reason(value: &[u8], state: &ExecutionState) -> Result<Vec<u8>, ExecutionError> {
    let contains_marker = value.windows(2).any(|marker| marker == b"${");
    if !state.variables_enabled || !contains_marker {
        return Ok(value.to_vec());
    }
    let text = std::str::from_utf8(value).map_err(|_| ExecutionError::VariableRuntime)?;
    state
        .variables
        .expand_octets(text, state.match_variables.octets())
        .map(String::into_bytes)
        .map_err(|_error| ExecutionError::VariableRuntime)
}

fn optional(value: Option<&str>, state: &ExecutionState) -> Result<Option<String>, ExecutionError> {
    value.map(|item| expand(item, state)).transpose()
}
