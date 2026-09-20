//! Ограниченные Sieve string comparators и glob matching без regex.

use super::ast::{Comparator, MatchType, StringMatch};

/// Сверяет одно значение со всеми script-ключами без привязки к protocol I/O.
pub(super) fn matches(value: &str, matcher: &StringMatch) -> bool {
    matcher
        .keys
        .iter()
        .any(|key| one(value, key, &matcher.match_type, &matcher.comparator))
}

fn one(value: &str, key: &str, kind: &MatchType, comparator: &Comparator) -> bool {
    match kind {
        MatchType::Is => equal(value, key, comparator),
        MatchType::Contains => contains(value, key, comparator),
        MatchType::Matches => glob(value, key, comparator),
    }
}

fn equal(value: &str, key: &str, comparator: &Comparator) -> bool {
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(key),
        Comparator::Octet => value == key,
    }
}

fn contains(value: &str, key: &str, comparator: &Comparator) -> bool {
    if key.is_empty() {
        return true;
    }
    match comparator {
        Comparator::AsciiCasemap => value
            .as_bytes()
            .windows(key.len())
            .any(|part| part.eq_ignore_ascii_case(key.as_bytes())),
        Comparator::Octet => value.contains(key),
    }
}

/// Bounded greedy glob для `*` и `?`, без экспоненциального backtracking.
fn glob(value: &str, pattern: &str, comparator: &Comparator) -> bool {
    let value: Vec<char> = value.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let mut value_position = 0_usize;
    let mut pattern_position = 0_usize;
    let mut star = None;
    let mut restart = None;
    while value.get(value_position).is_some() {
        match pattern.get(pattern_position).copied() {
            Some('*') => {
                star = Some(pattern_position);
                pattern_position = pattern_position.saturating_add(1);
                restart = Some(value_position);
            }
            Some('?') => {
                value_position = value_position.saturating_add(1);
                pattern_position = pattern_position.saturating_add(1);
            }
            Some(expected) if equal_character(value.get(value_position), expected, comparator) => {
                value_position = value_position.saturating_add(1);
                pattern_position = pattern_position.saturating_add(1);
            }
            _ => match (star, restart) {
                (Some(star_position), Some(restart_position)) => {
                    pattern_position = star_position.saturating_add(1);
                    let next_restart = restart_position.saturating_add(1);
                    restart = Some(next_restart);
                    value_position = next_restart;
                }
                _ => return false,
            },
        }
    }
    while matches!(pattern.get(pattern_position), Some('*')) {
        pattern_position = pattern_position.saturating_add(1);
    }
    pattern.get(pattern_position).is_none()
}

fn equal_character(value: Option<&char>, expected: char, comparator: &Comparator) -> bool {
    let Some(value) = value else {
        return false;
    };
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(&expected),
        Comparator::Octet => *value == expected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(matches("Invoice-2026", &insensitive));
        assert!(!matches("Invoice-2026", &octet));
    }
}
