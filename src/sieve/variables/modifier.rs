//! Реализация порядка и преобразований RFC 5229 для `set` modifiers.

use super::super::ast::VariableModifier;
use super::VariableError;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PrecedenceGroup {
    Case,
    FirstCharacter,
    WildcardQuote,
    Length,
}

impl VariableModifier {
    /// Возвращает приоритет применения из RFC 5229 §4.1.
    fn precedence(self) -> u8 {
        match self {
            Self::Lower | Self::Upper => 40,
            Self::LowerFirst | Self::UpperFirst => 30,
            Self::QuoteWildcard => 20,
            Self::Length => 10,
        }
    }

    /// Разделяет modifiers, конфликтующие на одном уровне приоритета.
    fn group(self) -> PrecedenceGroup {
        match self {
            Self::Lower | Self::Upper => PrecedenceGroup::Case,
            Self::LowerFirst | Self::UpperFirst => PrecedenceGroup::FirstCharacter,
            Self::QuoteWildcard => PrecedenceGroup::WildcardQuote,
            Self::Length => PrecedenceGroup::Length,
        }
    }
}

/// Проверяет конфликты и применяет modifiers в нормативном порядке.
pub(super) fn apply(source: &str, modifiers: &[VariableModifier]) -> Result<String, VariableError> {
    validate_unique_groups(modifiers)?;
    let mut ordered = modifiers.to_vec();
    ordered.sort_by_key(|modifier| std::cmp::Reverse(modifier.precedence()));
    let mut value = source.to_owned();
    for modifier in ordered {
        value = apply_one(&value, modifier);
    }
    Ok(value)
}

/// RFC 5229 запрещает больше одного modifier-а одной группы precedence.
pub(super) fn validate_unique_groups(modifiers: &[VariableModifier]) -> Result<(), VariableError> {
    let mut groups = HashSet::new();
    for modifier in modifiers {
        if !groups.insert(modifier.group()) {
            return Err(VariableError::InvalidModifiers);
        }
    }
    Ok(())
}

/// Выполняет ровно одно преобразование строки.
fn apply_one(source: &str, modifier: VariableModifier) -> String {
    match modifier {
        VariableModifier::Lower => source.to_ascii_lowercase(),
        VariableModifier::Upper => source.to_ascii_uppercase(),
        VariableModifier::LowerFirst => first_character(source, char::to_ascii_lowercase),
        VariableModifier::UpperFirst => first_character(source, char::to_ascii_uppercase),
        VariableModifier::QuoteWildcard => quote_wildcards(source),
        VariableModifier::Length => source.chars().count().to_string(),
    }
}

/// Меняет регистр только первого Unicode scalar value, оставляя его остаток точным.
fn first_character(source: &str, transform: fn(&char) -> char) -> String {
    let mut characters = source.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let mut result = String::with_capacity(source.len());
    result.push(transform(&first));
    result.extend(characters);
    result
}

/// Экранирует все glob metacharacters для последующего Sieve `:matches`.
fn quote_wildcards(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    for character in source.chars() {
        match character {
            '*' | '?' | '\\' => result.push('\\'),
            _ => {}
        }
        result.push(character);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_follow_rfc_precedence_instead_of_source_order() {
        let output = apply(
            "juMBlEd lETteRS",
            &[VariableModifier::UpperFirst, VariableModifier::Lower],
        )
        .expect("distinct modifier levels");

        assert_eq!(output, "Jumbled letters");
    }

    #[test]
    fn length_counts_unicode_characters_and_case_conversion_is_ascii_only() {
        let output = apply("éA", &[VariableModifier::Upper, VariableModifier::Length])
            .expect("distinct modifier levels");

        assert_eq!(output, "2");
        assert_eq!(apply("éA", &[VariableModifier::Upper]).unwrap(), "éA");
    }

    #[test]
    fn wildcard_quoting_escapes_star_question_and_backslash() {
        assert_eq!(
            apply("a*?\\b", &[VariableModifier::QuoteWildcard]).unwrap(),
            "a\\*\\?\\\\b"
        );
    }

    #[test]
    fn same_precedence_modifiers_are_rejected() {
        assert_eq!(
            apply("value", &[VariableModifier::Upper, VariableModifier::Lower]),
            Err(VariableError::InvalidModifiers)
        );
        assert_eq!(
            apply(
                "value",
                &[VariableModifier::UpperFirst, VariableModifier::LowerFirst]
            ),
            Err(VariableError::InvalidModifiers)
        );
    }
}
