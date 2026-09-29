//! Оценка Sieve tests и per-execution match-variable state.

use super::ExecutionError;
use crate::sieve::address;
use crate::sieve::ast::{AddressPart, Condition, EnvelopePart, SizeRelation, StringMatch};
use crate::sieve::matching::{self, MatchOutcome};
use crate::sieve::variables::VariableStore;
#[path = "execution_condition_variables.rs"]
mod variable_arguments;

/// Неизменяемый контекст одного сообщения и его SMTP envelope.
pub(super) struct MessageContext<'a> {
    pub(super) headers: &'a [(String, String)],
    pub(super) size: usize,
    pub(super) envelope_from: &'a str,
    pub(super) envelope_to: &'a str,
    pub(super) variables_enabled: bool,
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
            let names = variable_arguments::expand_strings(
                names,
                store,
                variables.octets(),
                context.variables_enabled,
            )?;
            let matcher = variable_arguments::expand_matcher(
                matcher,
                store,
                variables.octets(),
                context.variables_enabled,
            )?;
            for (candidate, value) in context.headers {
                if header_matches(candidate, value, &names, &matcher, variables)? {
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
            let names = variable_arguments::expand_strings(
                names,
                store,
                variables.octets(),
                context.variables_enabled,
            )?;
            let matcher = variable_arguments::expand_matcher(
                matcher,
                store,
                variables.octets(),
                context.variables_enabled,
            )?;
            for (candidate, value) in context.headers {
                if address_matches(candidate, value, &names, *part, &matcher, variables)? {
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
            let matcher = variable_arguments::expand_matcher(
                matcher,
                store,
                variables.octets(),
                context.variables_enabled,
            )?;
            for selector in parts {
                let selected = variable_arguments::expand_envelope_part(
                    selector,
                    store,
                    variables.octets(),
                    context.variables_enabled,
                )?;
                let value = match selected {
                    EnvelopePart::From => context.envelope_from,
                    EnvelopePart::To => context.envelope_to,
                };
                for entry in address::envelope_value(value, *part) {
                    if match_value(&entry, &matcher, variables)? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        Condition::String { sources, matcher } => {
            string_matches(sources, matcher, store, variables)
        }
        Condition::Exists { names } => {
            let names = variable_arguments::expand_strings(
                names,
                store,
                variables.octets(),
                context.variables_enabled,
            )?;
            Ok(names.iter().all(|name| {
                context
                    .headers
                    .iter()
                    .any(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            }))
        }
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
    sources: &[Vec<u8>],
    matcher: &StringMatch,
    store: &VariableStore,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    let expanded_sources =
        variable_arguments::expand_octet_strings(sources, store, variables.octets(), true)?;
    let expanded_matcher =
        variable_arguments::expand_matcher(matcher, store, variables.octets(), true)?;
    for source in &expanded_sources {
        if match_octets(source, &expanded_matcher, variables)? {
            return Ok(true);
        }
    }
    Ok(false)
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
    match_octets(value.as_bytes(), matcher, variables)
}

fn match_octets(
    value: &[u8],
    matcher: &StringMatch,
    variables: &mut MatchVariables,
) -> Result<bool, ExecutionError> {
    let outcome = matching::evaluate_octets(value, matcher).map_err(|error| match error {
        matching::MatchError::WorkLimit => ExecutionError::MatcherLimit,
        matching::MatchError::UnresolvedComparator => ExecutionError::VariableRuntime,
    })?;
    match outcome {
        MatchOutcome::NoMatch => Ok(false),
        MatchOutcome::Match => Ok(true),
        MatchOutcome::Captured(captures) => {
            variables.replace(value, captures);
            Ok(true)
        }
    }
}

#[cfg(test)]
#[path = "execution_condition_tests.rs"]
mod tests;
