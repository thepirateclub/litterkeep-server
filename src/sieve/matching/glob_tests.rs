//! Проверки RFC 5228 glob и RFC 5229 byte-captures.

use super::*;

#[test]
fn captures_are_shortest_and_preserve_pattern_order() {
    let result = captures("abcxdefx", "*x*", &Comparator::Octet).expect("match");
    assert_eq!(result, vec![b"abc".to_vec(), b"defx".to_vec()]);
}

#[test]
fn question_capture_is_one_octet_for_base_comparators() {
    for comparator in [Comparator::Octet, Comparator::AsciiCasemap] {
        assert_eq!(
            captures("é", "??", &comparator).expect("two UTF-8 octets"),
            vec![vec![0xc3], vec![0xa9]]
        );
    }
}

#[test]
fn escaped_wildcard_is_literal_and_not_captured() {
    assert_eq!(
        captures("invoice*", r"invoice\*", &Comparator::Octet),
        Some(Vec::new())
    );
    assert_eq!(
        captures("invoice-2026", r"invoice\*", &Comparator::Octet),
        None
    );
}

#[test]
fn failed_prefix_can_expand_the_latest_star_without_losing_captures() {
    let result = captures("abx-tail", "*b*tail", &Comparator::Octet).expect("match");
    assert_eq!(result, vec![b"a".to_vec(), b"x-".to_vec()]);
}

#[test]
fn retrying_an_earlier_star_discards_stale_later_captures() {
    let result = captures("abx-tail", "*b*x*tail", &Comparator::Octet).expect("match");
    assert_eq!(result, vec![b"a".to_vec(), b"".to_vec(), b"-".to_vec()]);
}

#[test]
fn retrying_the_first_star_recomputes_all_following_captures() {
    let result = captures("a--x-c", "*-x-*", &Comparator::Octet).expect("match");
    assert_eq!(result, vec![b"a-".to_vec(), b"c".to_vec()]);
}

#[test]
fn multiple_stars_retain_shortest_capture_partition() {
    let result = captures("a-b-c", "*-*-*", &Comparator::Octet).expect("match");
    assert_eq!(result, vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}
