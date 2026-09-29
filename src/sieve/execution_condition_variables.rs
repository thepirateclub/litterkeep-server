//! Раскрывает строковые аргументы test-ов при достижении условия.

use super::ExecutionError;
use crate::sieve::ast::{Comparator, EnvelopePart, EnvelopePartSelector, MatchType, StringMatch};
use crate::sieve::keywords::{ComparatorWord, EnvelopePartWord};
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
    let keys = expand_octet_strings(&matcher.keys, store, captures, enabled)?;
    let comparator = match &matcher.comparator {
        Comparator::AsciiCasemap => Comparator::AsciiCasemap,
        Comparator::Octet => Comparator::Octet,
        Comparator::Variable(value) if enabled => {
            let expanded = store
                .expand_octets(value, captures)
                .map_err(variable_error)?;
            match ComparatorWord::parse(&expanded) {
                ComparatorWord::AsciiCasemap => Comparator::AsciiCasemap,
                ComparatorWord::Octet => Comparator::Octet,
                ComparatorWord::Unknown => return Err(ExecutionError::VariableRuntime),
            }
        }
        Comparator::Variable(_) => return Err(ExecutionError::VariableRuntime),
    };
    Ok(StringMatch {
        match_type: clone_match_type(&matcher.match_type),
        comparator,
        keys,
    })
}

/// Раскрывает matcher values только после строгого перехода к UTF-8 при Variables.
pub(super) fn expand_octet_strings(
    values: &[Vec<u8>],
    store: &VariableStore,
    captures: &[Vec<u8>],
    enabled: bool,
) -> Result<Vec<Vec<u8>>, ExecutionError> {
    values
        .iter()
        .map(|value| match enabled {
            false => Ok(value.clone()),
            true if !value.windows(2).any(|marker| marker == b"${") => Ok(value.clone()),
            true => {
                let text =
                    std::str::from_utf8(value).map_err(|_| ExecutionError::VariableRuntime)?;
                store
                    .expand_octets(text, captures)
                    .map(|expanded| expanded.into_bytes())
                    .map_err(variable_error)
            }
        })
        .collect()
}

/// Раскрывает envelope-part string и сверяет результат с поддерживаемым enum.
pub(super) fn expand_envelope_part(
    selector: &EnvelopePartSelector,
    store: &VariableStore,
    captures: &[Vec<u8>],
    enabled: bool,
) -> Result<EnvelopePart, ExecutionError> {
    match selector {
        EnvelopePartSelector::Static(part) => Ok(*part),
        EnvelopePartSelector::Variable(source) if enabled => {
            let expanded = store
                .expand_octets(source, captures)
                .map_err(variable_error)?;
            match EnvelopePartWord::parse(&expanded) {
                EnvelopePartWord::From => Ok(EnvelopePart::From),
                EnvelopePartWord::To => Ok(EnvelopePart::To),
                EnvelopePartWord::Unknown => Err(ExecutionError::VariableRuntime),
            }
        }
        EnvelopePartSelector::Variable(_) => Err(ExecutionError::VariableRuntime),
    }
}

fn clone_match_type(value: &MatchType) -> MatchType {
    match value {
        MatchType::Is => MatchType::Is,
        MatchType::Contains => MatchType::Contains,
        MatchType::Matches => MatchType::Matches,
    }
}

pub(super) fn variable_error(_error: VariableError) -> ExecutionError {
    ExecutionError::VariableRuntime
}
