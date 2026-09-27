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
        Comparator::AsciiCasemap => contains_ascii_casemap(value, key),
        Comparator::Octet => value.contains(key),
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

/// Сопоставляет glob по UTF-8 octets, как требуют штатные comparator-ы Sieve.
fn glob(value: &str, pattern: &str, comparator: &Comparator) -> bool {
    let value = value.as_bytes();
    let pattern = glob_tokens(pattern);
    let mut value_position = 0_usize;
    let mut pattern_position = 0_usize;
    let mut star = None;
    let mut restart = None;
    while value.get(value_position).is_some() {
        match pattern.get(pattern_position).copied() {
            Some(GlobToken::Many) => {
                star = Some(pattern_position);
                pattern_position = pattern_position.saturating_add(1);
                restart = Some(value_position);
            }
            Some(GlobToken::One) => {
                value_position = value_position.saturating_add(1);
                pattern_position = pattern_position.saturating_add(1);
            }
            Some(GlobToken::Literal(expected))
                if value
                    .get(value_position)
                    .is_some_and(|actual| equal_octet(*actual, expected, comparator)) =>
            {
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
    while matches!(pattern.get(pattern_position), Some(GlobToken::Many)) {
        pattern_position = pattern_position.saturating_add(1);
    }
    pattern.get(pattern_position).is_none()
}

/// Типизированный glob-token, чтобы escaped wildcard оставался буквальным.
#[derive(Clone, Copy)]
enum GlobToken {
    Literal(u8),
    One,
    Many,
}

/// Преобразует UTF-8 pattern в octet tokens без прямого индексирования.
fn glob_tokens(pattern: &str) -> Vec<GlobToken> {
    let mut bytes = pattern.as_bytes().iter().copied().peekable();
    let mut tokens = Vec::new();
    while let Some(byte) = bytes.next() {
        let token = match (byte, bytes.peek().copied()) {
            (b'\\', Some(b'*' | b'?')) => match bytes.next() {
                Some(value) => GlobToken::Literal(value),
                None => GlobToken::Literal(byte),
            },
            (b'*', _) => GlobToken::Many,
            (b'?', _) => GlobToken::One,
            _ => GlobToken::Literal(byte),
        };
        tokens.push(token);
    }
    tokens
}

/// Применяет octet comparator или ASCII-only case folding к одной byte.
fn equal_octet(value: u8, expected: u8, comparator: &Comparator) -> bool {
    match comparator {
        Comparator::AsciiCasemap => value.eq_ignore_ascii_case(&expected),
        Comparator::Octet => value == expected,
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
            assert!(!matches("é", &one_octet));
            assert!(matches("é", &two_octets));
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
        assert!(matches("invoice*", &star));
        assert!(!matches("invoice-2026", &star));
        assert!(matches("invoice?", &question));
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

        assert!(!matches(&value, &matcher));
    }

    /// KMP сравнивает только ASCII case; прочие UTF-8 octets остаются точными.
    #[test]
    fn contains_casemap_folds_ascii_without_changing_non_ascii_octets() {
        assert!(contains("INVOICE-2026", "voice", &Comparator::AsciiCasemap));
        assert!(!contains("École", "éCOLE", &Comparator::AsciiCasemap));
        assert!(contains("École", "Éco", &Comparator::AsciiCasemap));
    }
}
