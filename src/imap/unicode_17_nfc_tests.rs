//! Регрессии NFC и полный официальный Unicode 17.0.0 conformance corpus.

use super::normalize;

#[test]
fn composes_canonical_latin_and_hangul_sequences() {
    assert_eq!(normalize("cafe\u{0301}"), "café");
    assert_eq!(normalize("\u{1100}\u{1161}\u{11A8}"), "각");
}

#[test]
fn canonically_reorders_marks_before_composition() {
    assert_eq!(normalize("a\u{0315}\u{0300}"), "à\u{0315}");
}

#[test]
fn composition_exclusion_stays_decomposed_and_is_idempotent() {
    let normalized = normalize("\u{0344}");
    assert_eq!(normalized, "\u{0308}\u{0301}");
    assert_eq!(normalize(&normalized), normalized);
}

/// Запускается отдельно с полным файлом из официального UCD 17.0.0.
#[test]
#[ignore = "run with LITTERKEEP_UNICODE_NORMALIZATION_TEST pointing to UCD NormalizationTest.txt"]
fn official_unicode_normalization_vectors() {
    use std::io::{BufRead, BufReader};

    let path = std::env::var("LITTERKEEP_UNICODE_NORMALIZATION_TEST")
        .expect("set path to official Unicode 17.0.0 NormalizationTest.txt");
    let file = std::fs::File::open(path).expect("open official normalization corpus");
    for (line_number, line_result) in BufReader::new(file).lines().enumerate() {
        let line = line_result.expect("read normalization vector");
        let (content, _) = line.split_once('#').unwrap_or((&line, ""));
        let content = content.trim();
        if content.is_empty() || content.starts_with('@') {
            continue;
        }
        let mut columns = content.split(';');
        let first = sequence(columns.next().expect("column c1"));
        let second = sequence(columns.next().expect("column c2"));
        let third = sequence(columns.next().expect("column c3"));
        let fourth = sequence(columns.next().expect("column c4"));
        let fifth = sequence(columns.next().expect("column c5"));
        for (input, expected) in [(&first, &second), (&second, &second), (&third, &second)] {
            assert_eq!(
                normalize(input),
                *expected,
                "NFC corpus line {}",
                line_number + 1
            );
        }
        for input in [&fourth, &fifth] {
            assert_eq!(
                normalize(input),
                fourth,
                "NFC corpus line {}",
                line_number + 1
            );
        }
    }
}

/// Преобразует UCD hex scalar sequence без прямого индексирования полей.
fn sequence(encoded: &str) -> String {
    encoded
        .split_whitespace()
        .map(|point| {
            u32::from_str_radix(point, 16)
                .ok()
                .and_then(char::from_u32)
                .expect("UCD vector contains a Unicode scalar")
        })
        .collect()
}
