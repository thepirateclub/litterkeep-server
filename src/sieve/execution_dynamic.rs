//! Общие преобразования динамических Sieve action arguments.

use super::{ExecutionError, ExecutionState};

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
