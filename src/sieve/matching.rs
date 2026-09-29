//! Ограниченные Sieve string comparators и glob matching без regex.

use super::ast::{Comparator, MatchType, StringMatch};
#[path = "matching/glob.rs"]
mod glob;

/// Результат сравнения сохраняет captures только успешного wildcard match-а.
pub(super) enum MatchOutcome {
    NoMatch,
    Match,
    Captured(Vec<Vec<u8>>),
}

/// Ошибка comparator-а, которую executor обязан передать в delivery layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MatchError {
    WorkLimit,
    UnresolvedComparator,
}

/// Сравнивает ключи по порядку, чтобы runtime получил captures успешного ключа.
pub(super) fn evaluate(value: &str, matcher: &StringMatch) -> Result<MatchOutcome, MatchError> {
    if matches!(matcher.comparator, Comparator::Variable(_)) {
        return Err(MatchError::UnresolvedComparator);
    }
    for key in &matcher.keys {
        let outcome = match &matcher.match_type {
            MatchType::Is => match equal(value, key, &matcher.comparator) {
                true => MatchOutcome::Match,
                false => MatchOutcome::NoMatch,
            },
            MatchType::Contains => match contains(value, key, &matcher.comparator) {
                true => MatchOutcome::Match,
                false => MatchOutcome::NoMatch,
            },
            MatchType::Matches => match glob::captures(value, key, &matcher.comparator) {
                Ok(Some(captures)) => MatchOutcome::Captured(captures),
                Ok(None) => MatchOutcome::NoMatch,
                Err(glob::GlobError::WorkLimit) => return Err(MatchError::WorkLimit),
            },
        };
        match outcome {
            MatchOutcome::NoMatch => {}
            result => return Ok(result),
        }
    }
    Ok(MatchOutcome::NoMatch)
}

fn equal(value: &str, key: &str, comparator: &Comparator) -> bool {
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(key),
        Comparator::Octet => value == key,
        Comparator::Variable(_) => false,
    }
}

fn contains(value: &str, key: &str, comparator: &Comparator) -> bool {
    if key.is_empty() {
        return true;
    }
    match comparator {
        Comparator::AsciiCasemap => contains_ascii_casemap(value, key),
        Comparator::Octet => value.contains(key),
        Comparator::Variable(_) => false,
    }
}

/// Выполняет ASCII-casemap поиск за O(n + m), не копируя всё поле заголовка.
fn contains_ascii_casemap(value: &str, key: &str) -> bool {
    let pattern = key
        .bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let prefix = prefix_lengths(&pattern);
    let mut matched = 0_usize;
    for byte in value.bytes().map(|value| value.to_ascii_lowercase()) {
        while matched > 0 && pattern.get(matched).copied() != Some(byte) {
            matched = prefix.get(matched.saturating_sub(1)).copied().unwrap_or(0);
        }
        match pattern.get(matched).copied() {
            Some(expected) if expected == byte => matched = matched.saturating_add(1),
            _ => {}
        }
        if matched == pattern.len() {
            return true;
        }
    }
    false
}

/// Строит KMP prefix table, сохраняя линейную стоимость последующих mismatches.
fn prefix_lengths(pattern: &[u8]) -> Vec<usize> {
    let mut prefix = Vec::with_capacity(pattern.len());
    prefix.push(0);
    for byte in pattern.iter().copied().skip(1) {
        let mut matched = prefix.last().copied().unwrap_or(0);
        while matched > 0 && pattern.get(matched).copied() != Some(byte) {
            matched = prefix.get(matched.saturating_sub(1)).copied().unwrap_or(0);
        }
        match pattern.get(matched).copied() {
            Some(expected) if expected == byte => matched = matched.saturating_add(1),
            _ => {}
        }
        prefix.push(matched);
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(value: &str, matcher: &StringMatch) -> MatchOutcome {
        evaluate(value, matcher).expect("bounded comparator work")
    }

    #[test]
    fn glob_is_case_policy_aware_without_backtracking() {
        let insensitive = StringMatch {
            match_type: MatchType::Matches,
            comparator: Comparator::AsciiCasemap,
            keys: vec!["inv*2026".into()],
        };
        let octet = StringMatch {
            comparator: Comparator::Octet,
            ..insensitive.clone()
        };
        assert!(!matches!(
            outcome("Invoice-2026", &insensitive),
            MatchOutcome::NoMatch
        ));
        assert!(matches!(
            outcome("Invoice-2026", &octet),
            MatchOutcome::NoMatch
        ));
    }

    #[test]
    fn wildcard_width_is_one_octet_for_both_base_comparators() {
        for comparator in [Comparator::Octet, Comparator::AsciiCasemap] {
            let one_octet = StringMatch {
                match_type: MatchType::Matches,
                comparator: comparator.clone(),
                keys: vec!["?".into()],
            };
            let two_octets = StringMatch {
                keys: vec!["??".into()],
                ..one_octet.clone()
            };
            assert!(matches!(outcome("é", &one_octet), MatchOutcome::NoMatch));
            assert!(!matches!(outcome("é", &two_octets), MatchOutcome::NoMatch));
        }
    }

    #[test]
    fn escaped_wildcards_are_compared_as_literal_octets() {
        let star = StringMatch {
            match_type: MatchType::Matches,
            comparator: Comparator::Octet,
            keys: vec![r"invoice\*".into()],
        };
        let question = StringMatch {
            keys: vec![r"invoice\?".into()],
            ..star.clone()
        };
        assert!(!matches!(outcome("invoice*", &star), MatchOutcome::NoMatch));
        assert!(matches!(
            outcome("invoice-2026", &star),
            MatchOutcome::NoMatch
        ));
        assert!(!matches!(
            outcome("invoice?", &question),
            MatchOutcome::NoMatch
        ));
    }

    /// Отрицательный ASCII-casemap поиск не должен сравнивать needle в каждом окне.
    #[test]
    fn long_ascii_contains_miss_is_bounded_by_input_and_key_length() {
        let value = "a".repeat(131_072);
        let key = format!("{}B", "A".repeat(2_048));
        let matcher = StringMatch {
            match_type: MatchType::Contains,
            comparator: Comparator::AsciiCasemap,
            keys: vec![key],
        };

        assert!(matches!(outcome(&value, &matcher), MatchOutcome::NoMatch));
    }

    /// KMP сравнивает только ASCII case; прочие UTF-8 octets остаются точными.
    #[test]
    fn contains_casemap_folds_ascii_without_changing_non_ascii_octets() {
        assert!(contains("INVOICE-2026", "voice", &Comparator::AsciiCasemap));
        assert!(!contains("École", "éCOLE", &Comparator::AsciiCasemap));
        assert!(contains("École", "Éco", &Comparator::AsciiCasemap));
    }
}
