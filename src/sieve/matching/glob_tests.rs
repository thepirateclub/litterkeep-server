//! Проверки RFC 5228 glob и RFC 5229 byte-captures.

use super::*;

#[test]
fn captures_are_shortest_and_preserve_pattern_order() {
    let result = captures("abcxdefx", "*x*", &Comparator::Octet)
        .expect("within work budget")
        .expect("match");
    assert_eq!(result, vec![b"abc".to_vec(), b"defx".to_vec()]);
}

#[test]
fn question_capture_is_one_octet_for_base_comparators() {
    for comparator in [Comparator::Octet, Comparator::AsciiCasemap] {
        assert_eq!(
            captures("é", "??", &comparator)
                .expect("within work budget")
                .expect("two UTF-8 octets"),
            vec![vec![0xc3], vec![0xa9]]
        );
    }
}

#[test]
fn escaped_wildcard_is_literal_and_not_captured() {
    assert_eq!(
        captures("invoice*", r"invoice\*", &Comparator::Octet),
        Ok(Some(Vec::new()))
    );
    assert_eq!(
        captures("invoice-2026", r"invoice\*", &Comparator::Octet),
        Ok(None)
    );
}

#[test]
fn failed_prefix_can_expand_the_latest_star_without_losing_captures() {
    let result = captures("abx-tail", "*b*tail", &Comparator::Octet)
        .expect("within work budget")
        .expect("match");
    assert_eq!(result, vec![b"a".to_vec(), b"x-".to_vec()]);
}

#[test]
fn retrying_an_earlier_star_discards_stale_later_captures() {
    let result = captures("abx-tail", "*b*x*tail", &Comparator::Octet)
        .expect("within work budget")
        .expect("match");
    assert_eq!(result, vec![b"a".to_vec(), b"".to_vec(), b"-".to_vec()]);
}

#[test]
fn retrying_the_first_star_recomputes_all_following_captures() {
    let result = captures("a--x-c", "*-x-*", &Comparator::Octet)
        .expect("within work budget")
        .expect("match");
    assert_eq!(result, vec![b"a-".to_vec(), b"c".to_vec()]);
}

#[test]
fn multiple_stars_retain_shortest_capture_partition() {
    let result = captures("a-b-c", "*-*-*", &Comparator::Octet)
        .expect("within work budget")
        .expect("match");
    assert_eq!(result, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}

#[test]
fn bounded_matcher_agrees_with_exhaustive_nongreedy_reference_corpus() {
    let patterns = words(b"ab*?", 5);
    let values = words(b"ab", 4);
    for pattern in &patterns {
        for value in &values {
            let pattern_text = std::str::from_utf8(pattern).expect("ASCII pattern");
            let value_text = std::str::from_utf8(value).expect("ASCII value");
            let expected = reference(pattern, value, 0, Vec::new());
            let actual = captures(value_text, pattern_text, &Comparator::Octet)
                .expect("small corpus stays within work budget");
            assert_eq!(
                actual, expected,
                "pattern {pattern_text:?}, value {value_text:?}"
            );
        }
    }
}

/// Строит все строки алфавита до заданной длины без предположений о layout.
fn words(alphabet: &[u8], maximum: usize) -> Vec<Vec<u8>> {
    let mut result = Vec::new();
    let mut prefix = Vec::new();
    collect_words(alphabet, maximum, &mut prefix, &mut result);
    result
}

fn collect_words(
    alphabet: &[u8],
    remaining: usize,
    prefix: &mut Vec<u8>,
    result: &mut Vec<Vec<u8>>,
) {
    result.push(prefix.clone());
    if remaining == 0 {
        return;
    }
    for byte in alphabet {
        prefix.push(*byte);
        collect_words(alphabet, remaining.saturating_sub(1), prefix, result);
        let _ = prefix.pop();
    }
}

/// Медленный oracle перебирает длины `*` по возрастанию, то есть non-greedy.
fn reference(
    pattern: &[u8],
    value: &[u8],
    value_position: usize,
    captures: Vec<Vec<u8>>,
) -> Option<Vec<Vec<u8>>> {
    let Some((token, rest)) = pattern.split_first() else {
        return match value_position == value.len() {
            true => Some(captures),
            false => None,
        };
    };
    match token {
        b'*' => reference_star(rest, value, value_position, captures),
        b'?' => reference_one(rest, value, value_position, captures),
        literal => match value.get(value_position) {
            Some(actual) if actual == literal => {
                reference(rest, value, value_position.saturating_add(1), captures)
            }
            _ => None,
        },
    }
}

fn reference_star(
    pattern: &[u8],
    value: &[u8],
    start: usize,
    captures: Vec<Vec<u8>>,
) -> Option<Vec<Vec<u8>>> {
    for end in start..=value.len() {
        let mut candidate = captures.clone();
        candidate.push(value.get(start..end)?.to_vec());
        if let Some(result) = reference(pattern, value, end, candidate) {
            return Some(result);
        }
    }
    None
}

fn reference_one(
    pattern: &[u8],
    value: &[u8],
    position: usize,
    mut captures: Vec<Vec<u8>>,
) -> Option<Vec<Vec<u8>>> {
    let end = position.saturating_add(1);
    let capture = value.get(position..end)?;
    captures.push(capture.to_vec());
    reference(pattern, value, end, captures)
}
