//! Обнаруживает dynamic selector arguments, требующие RFC 5229 Variables.

use super::super::ast::{Comparator, Condition, EnvelopePartSelector};

/// Проверяет всё дерево условий до публикации parsed script-а.
pub(super) fn has_string_selector(condition: &Condition) -> bool {
    match condition {
        Condition::Header { matcher, .. }
        | Condition::Address { matcher, .. }
        | Condition::String { matcher, .. } => dynamic_comparator(&matcher.comparator),
        Condition::Envelope { parts, matcher, .. } => {
            dynamic_comparator(&matcher.comparator) || parts.iter().any(dynamic_envelope_part)
        }
        Condition::Not(nested) => has_string_selector(nested),
        Condition::AllOf(values) | Condition::AnyOf(values) => {
            values.iter().any(has_string_selector)
        }
        Condition::True | Condition::False | Condition::Exists { .. } | Condition::Size { .. } => {
            false
        }
    }
}

fn dynamic_comparator(value: &Comparator) -> bool {
    matches!(value, Comparator::Variable(_))
}

fn dynamic_envelope_part(value: &EnvelopePartSelector) -> bool {
    matches!(value, EnvelopePartSelector::Variable(_))
}
