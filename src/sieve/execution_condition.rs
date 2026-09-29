//! Оценка Sieve tests и per-execution match-variable state.

use crate::sieve::address;
use crate::sieve::ast::{AddressPart, Condition, EnvelopePart, SizeRelation, StringMatch};
use crate::sieve::matching::{self, MatchOutcome};

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
    variables: &mut MatchVariables,
) -> bool {
    match condition {
        Condition::True => true,
        Condition::False => false,
        Condition::Header { names, matcher } => context
            .headers
            .iter()
            .any(|(candidate, value)| header_matches(candidate, value, names, matcher, variables)),
        Condition::Address {
            names,
            part,
            matcher,
        } => context.headers.iter().any(|(candidate, value)| {
            address_matches(candidate, value, names, *part, matcher, variables)
        }),
        Condition::Envelope {
            parts,
            part,
            matcher,
        } => parts.iter().any(|selected| {
            let value = match selected {
                EnvelopePart::From => context.envelope_from,
                EnvelopePart::To => context.envelope_to,
            };
            address::envelope_value(value, *part)
                .iter()
                .any(|entry| match_value(entry, matcher, variables))
        }),
        Condition::Exists { names } => names.iter().all(|name| {
            context
                .headers
                .iter()
                .any(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        }),
        Condition::Size { relation, limit } => match relation {
            SizeRelation::Over => context.size > *limit,
            SizeRelation::Under => context.size < *limit,
        },
        Condition::Not(value) => !matches(value, context, variables),
        Condition::AllOf(values) => values
            .iter()
            .all(|value| matches(value, context, variables)),
        Condition::AnyOf(values) => values
            .iter()
            .any(|value| matches(value, context, variables)),
    }
}

fn address_matches(
    candidate: &str,
    value: &str,
    names: &[String],
    part: AddressPart,
    matcher: &StringMatch,
    variables: &mut MatchVariables,
) -> bool {
    let kind = crate::mime::header_block::HeaderName::parse(candidate);
    if !kind.is_address_list() {
        return false;
    }
    if !names
        .iter()
        .any(|name| crate::mime::header_block::HeaderName::parse(name) == kind)
    {
        return false;
    }
    address::values(value, part)
        .iter()
        .any(|entry| match_value(entry, matcher, variables))
}

/// Проверяет имя header и его декодированное значение в одном проходе.
fn header_matches(
    candidate: &str,
    value: &str,
    names: &[String],
    matcher: &StringMatch,
    variables: &mut MatchVariables,
) -> bool {
    if !names
        .iter()
        .any(|name| candidate.eq_ignore_ascii_case(name))
    {
        return false;
    }
    let normalized = value.trim_matches([' ', '\t']);
    match crate::mime::search_text::decode_header_value(normalized.as_bytes()) {
        Some(decoded) => match_value(&decoded, matcher, variables),
        None => false,
    }
}

/// Обновляет match variables только для successful wildcard comparator.
fn match_value(value: &str, matcher: &StringMatch, variables: &mut MatchVariables) -> bool {
    match matching::evaluate(value, matcher) {
        MatchOutcome::NoMatch => false,
        MatchOutcome::Match => true,
        MatchOutcome::Captured(captures) => {
            variables.replace(value.as_bytes(), captures);
            true
        }
    }
}

#[cfg(test)]
#[path = "execution_condition_tests.rs"]
mod tests;
