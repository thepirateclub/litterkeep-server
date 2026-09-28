//! RFC 4013 profile vectors and complete Unicode 3.2 normalization corpus test.

use super::{SaslPrepContext, SaslPrepError, prepare};

#[test]
fn rfc_4013_mapping_and_nfkc_examples_match() {
    assert_eq!(
        prepare("I\u{00AD}X", SaslPrepContext::StoredValue),
        Ok("IX".to_owned())
    );
    assert_eq!(
        prepare("user", SaslPrepContext::StoredValue),
        Ok("user".to_owned())
    );
    assert_eq!(
        prepare("USER", SaslPrepContext::StoredValue),
        Ok("USER".to_owned())
    );
    assert_eq!(
        prepare("\u{00AA}", SaslPrepContext::StoredValue),
        Ok("a".to_owned())
    );
    assert_eq!(
        prepare("\u{2168}", SaslPrepContext::StoredValue),
        Ok("IX".to_owned())
    );
}

#[test]
fn rfc_4013_rejects_prohibited_and_invalid_bidi_examples() {
    assert_eq!(
        prepare("\u{0007}", SaslPrepContext::Query),
        Err(SaslPrepError::ProhibitedCodePoint)
    );
    assert_eq!(
        prepare("\u{0627}1", SaslPrepContext::Query),
        Err(SaslPrepError::BidirectionalRule)
    );
}

#[test]
fn non_ascii_spaces_map_to_ascii_space_before_nfkc() {
    assert_eq!(
        prepare("left\u{00A0}right", SaslPrepContext::StoredValue),
        Ok("left right".to_owned())
    );
}

#[test]
fn unassigned_code_points_are_context_sensitive_as_rfc_3454_requires() {
    assert_eq!(
        prepare("\u{0221}", SaslPrepContext::StoredValue),
        Err(SaslPrepError::UnassignedCodePoint)
    );
    assert_eq!(
        prepare("\u{0221}", SaslPrepContext::Query),
        Ok("\u{0221}".to_owned())
    );
}

/// Сверяет NFKC всех колонок полного официального Unicode 3.2 corpus.
#[test]
#[ignore = "set LITTERKEEP_UNICODE_32_NORMALIZATION_TEST to the official corpus path"]
fn official_unicode_32_nfkc_vectors() {
    use std::io::{BufRead, BufReader};

    let path = std::env::var("LITTERKEEP_UNICODE_32_NORMALIZATION_TEST")
        .expect("set the official Unicode 3.2 NormalizationTest.txt path");
    let file = std::fs::File::open(path).expect("open pinned Unicode 3.2 corpus");
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
        for input in [&first, &second, &third] {
            assert_eq!(
                normalize_for_test(input),
                fourth,
                "NFKC line {}",
                line_number + 1
            );
        }
        for input in [&fourth, &fifth] {
            assert_eq!(
                normalize_for_test(input),
                fourth,
                "NFKC line {}",
                line_number + 1
            );
        }
    }
}

/// Вызывает изолированный Unicode 3.2 normalizer для corpus assertions.
fn normalize_for_test(value: &str) -> String {
    super::nfkc::normalize(value)
}

/// Декодирует одну официальную колонку hexadecimal scalar values.
fn sequence(encoded: &str) -> String {
    encoded
        .split_whitespace()
        .map(|point| {
            u32::from_str_radix(point, 16)
                .ok()
                .and_then(char::from_u32)
                .expect("official corpus scalar")
        })
        .collect()
}
