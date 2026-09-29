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
/// Сравнивает RFC 5228 строки как октеты, не требуя валидного UTF-8 payload.
pub(super) fn evaluate_octets(
    value: &[u8],
    matcher: &StringMatch,
) -> Result<MatchOutcome, MatchError> {
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

fn equal(value: &[u8], key: &[u8], comparator: &Comparator) -> bool {
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(key),
        Comparator::Octet => value == key,
        Comparator::Variable(_) => false,
    }
}

fn contains(value: &[u8], key: &[u8], comparator: &Comparator) -> bool {
    if key.is_empty() {
        return true;
    }
    match comparator {
        Comparator::AsciiCasemap => contains_folded(value, key, true),
        Comparator::Octet => contains_folded(value, key, false),
        Comparator::Variable(_) => false,
    }
}

/// Выполняет линейный поиск и при необходимости приводит только ASCII-регистр.
fn contains_folded(value: &[u8], key: &[u8], ascii_case_insensitive: bool) -> bool {
    if key.is_empty() {
        return true;
    }
    let pattern = key
        .iter()
        .copied()
        .map(|byte| fold_ascii(byte, ascii_case_insensitive))
        .collect::<Vec<_>>();
    let prefix = prefix_lengths(&pattern);
    let mut matched = 0_usize;
    for byte in value
        .iter()
        .map(|value| fold_ascii(*value, ascii_case_insensitive))
    {
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

fn fold_ascii(value: u8, enabled: bool) -> u8 {
    match enabled {
        true => value.to_ascii_lowercase(),
        false => value,
    }
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
        evaluate_octets(value.as_bytes(), matcher).expect("bounded comparator work")
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
            keys: vec![key.into_bytes()],
        };

        assert!(matches!(outcome(&value, &matcher), MatchOutcome::NoMatch));
    }

    /// KMP сравнивает только ASCII case; прочие UTF-8 octets остаются точными.
    #[test]
    fn contains_casemap_folds_ascii_without_changing_non_ascii_octets() {
        assert!(contains_folded(b"INVOICE-2026", b"voice", true));
        assert!(!contains_folded(
            "École".as_bytes(),
            "éCOLE".as_bytes(),
            true
        ));
        assert!(contains_folded("École".as_bytes(), "Éco".as_bytes(), true));
    }

    #[test]
    fn octet_comparator_matches_invalid_utf8_without_conversion() {
        let matcher = StringMatch {
            match_type: MatchType::Is,
            comparator: Comparator::Octet,
            keys: vec![b"A\xff\x80Z".to_vec()],
        };
        assert!(matches!(
            evaluate_octets(b"A\xff\x80Z", &matcher),
            Ok(MatchOutcome::Match)
        ));
        assert!(matches!(
            evaluate_octets(b"A\xffZ", &matcher),
            Ok(MatchOutcome::NoMatch)
        ));
    }
}
