//! Оценка Sieve tests и per-execution match-variable state.

use super::ExecutionError;
use crate::sieve::address;
use crate::sieve::ast::{AddressPart, Condition, EnvelopePart, SizeRelation, StringMatch};
use crate::sieve::matching::{self, MatchOutcome};
use crate::sieve::variables::{VariableError, VariableStore};

/// Неизменяемый контекст одного сообщения и его SMTP envelope.
pub(super) struct MessageContext<'a> {
    pub(super) headers: &'a [(String, String)],
    pub(super) size: usize,
    pub(super) envelope_from: &'a str,
    pub(super) envelope_to: &'a str,
}

/// Captures последнего успешного `:matches` в текущем script execution.
#[derive(Default)]
pub(super) struct MatchVariables {
    values: Vec<Vec<u8>>,
}

impl MatchVariables {
    /// Заменяет capture set только после успешного `:matches` сравнения.
    fn replace(&mut self, source: &[u8], wildcards: Vec<Vec<u8>>) {
        self.values.clear();
        self.values.push(source.to_vec());
        self.values.extend(wildcards);
    }

    /// Предоставляет byte captures, чтобы Variables декодировал только запрошенную ссылку.
    pub(super) fn octets(&self) -> &[Vec<u8>] {
        &self.values
    }

    /// Возвращает capture по RFC индексу, не индексируя коллекцию напрямую.
    #[cfg(test)]
    fn get(&self, index: usize) -> Option<&[u8]> {
        self.values.get(index).map(Vec::as_slice)
    }
}

/// Вычисляет условие слева направо, сохраняя captures успешных `:matches`.
pub(super) fn matches(
    condition: &Condition,
    context: &MessageContext<'_>,
    store: &VariableStore,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    match condition {
        Condition::True => Ok(true),
        Condition::False => Ok(false),
        Condition::Header { names, matcher } => {
            for (candidate, value) in context.headers {
                if header_matches(candidate, value, names, matcher, variables)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Address {
            names,
            part,
            matcher,
        } => {
            for (candidate, value) in context.headers {
                if address_matches(candidate, value, names, *part, matcher, variables)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Condition::Envelope {
            parts,
            part,
            matcher,
        } => {
            for selected in parts {
                let value = match selected {
                    EnvelopePart::From => context.envelope_from,
                    EnvelopePart::To => context.envelope_to,
                };
                for entry in address::envelope_value(value, *part) {
                    if match_value(&entry, matcher, variables)? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        Condition::String { sources, matcher } => {
            string_matches(sources, matcher, store, variables)
        }
        Condition::Exists { names } => Ok(names.iter().all(|name| {
            context
                .headers
                .iter()
                .any(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        })),
        Condition::Size { relation, limit } => Ok(match relation {
            SizeRelation::Over => context.size > *limit,
            SizeRelation::Under => context.size < *limit,
        }),
        Condition::Not(value) => Ok(!matches(value, context, store, variables)?),
        Condition::AllOf(values) => {
            for value in values {
                if !matches(value, context, store, variables)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Condition::AnyOf(values) => {
            for value in values {
                if matches(value, context, store, variables)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

/// Раскрывает оба списка в точке теста и сохраняет capture только успешного ключа.
fn string_matches(
    sources: &[String],
    matcher: &StringMatch,
    store: &VariableStore,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    let captures = variables.octets().to_vec();
    for source in sources {
        let expanded_source = store
            .expand_octets(source, &captures)
            .map_err(variable_error)?;
        for key in &matcher.keys {
            let expanded_key = store
                .expand_octets(key, &captures)
                .map_err(variable_error)?;
            let expanded_matcher = StringMatch {
                match_type: matcher.match_type.clone(),
                comparator: matcher.comparator.clone(),
                keys: vec![expanded_key],
            };
            if match_value(&expanded_source, &expanded_matcher, variables)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn variable_error(_error: VariableError) -> ExecutionError {
    ExecutionError::VariableRuntime
}

fn address_matches(
    candidate: &str,
    value: &str,
    names: &[String],
    part: AddressPart,
    matcher: &StringMatch,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    let kind = crate::mime::header_block::HeaderName::parse(candidate);
    if !kind.is_address_list() {
        return Ok(false);
    }
    if !names
        .iter()
        .any(|name| crate::mime::header_block::HeaderName::parse(name) == kind)
    {
        return Ok(false);
    }
    for entry in address::values(value, part) {
        if match_value(&entry, matcher, variables)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Проверяет имя header и его декодированное значение в одном проходе.
fn header_matches(
    candidate: &str,
    value: &str,
    names: &[String],
    matcher: &StringMatch,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    if !names
        .iter()
        .any(|name| candidate.eq_ignore_ascii_case(name))
    {
        return Ok(false);
    }
    let normalized = value.trim_matches([' ', '\t']);
    match crate::mime::search_text::decode_header_value(normalized.as_bytes()) {
        Some(decoded) => match_value(&decoded, matcher, variables),
        None => Ok(false),
    }
}

/// Обновляет match variables только для successful wildcard comparator.
fn match_value(
    value: &str,
    matcher: &StringMatch,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    let outcome = matching::evaluate(value, matcher).map_err(|error| match error {
        matching::MatchError::WorkLimit => ExecutionError::MatcherLimit,
    })?;
    match outcome {
        MatchOutcome::NoMatch => Ok(false),
        MatchOutcome::Match => Ok(true),
        MatchOutcome::Captured(captures) => {
            variables.replace(value.as_bytes(), captures);
            Ok(true)
        }
    }
}

#[cfg(test)]
#[path = "execution_condition_tests.rs"]
mod tests;
