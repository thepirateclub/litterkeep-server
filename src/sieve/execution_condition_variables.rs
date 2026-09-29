//! Раскрывает строковые аргументы test-ов при достижении условия.

use super::ExecutionError;
use crate::sieve::ast::{Comparator, MatchType, StringMatch};
use crate::sieve::variables::{VariableError, VariableStore};

/// Раскрывает все элементы списка с одним снимком match variables.
pub(super) fn expand_strings(
    values: &[String],
    store: &VariableStore,
    captures: &[Vec<u8>],
    enabled: bool,
) -> Result<Vec<String>, ExecutionError> {
    if !enabled {
        return Ok(values.to_vec());
    }
    values
        .iter()
        .map(|value| store.expand_octets(value, captures).map_err(variable_error))
        .collect()
}

/// Сохраняет статические comparator-ы и раскрывает каждый ключ до сравнения.
pub(super) fn expand_matcher(
    matcher: &StringMatch,
    store: &VariableStore,
    captures: &[Vec<u8>],
    enabled: bool,
) -> Result<StringMatch, ExecutionError> {
    let keys = expand_strings(&matcher.keys, store, captures, enabled)?;
    Ok(StringMatch {
        match_type: clone_match_type(&matcher.match_type),
        comparator: clone_comparator(&matcher.comparator),
        keys,
    })
}

fn clone_match_type(value: &MatchType) -> MatchType {
    match value {
        MatchType::Is => MatchType::Is,
        MatchType::Contains => MatchType::Contains,
        MatchType::Matches => MatchType::Matches,
    }
}

fn clone_comparator(value: &Comparator) -> Comparator {
    match value {
        Comparator::AsciiCasemap => Comparator::AsciiCasemap,
        Comparator::Octet => Comparator::Octet,
    }
}

pub(super) fn variable_error(_error: VariableError) -> ExecutionError {
    ExecutionError::VariableRuntime
}
