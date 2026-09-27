//! Сверка IDNA Derived Property со всем официальным Unicode 17 corpus.

use super::{DerivedProperty, derived_property};
use std::{env, fs};

/// Проверяет таблицу каждого Unicode scalar по закреплённому oracle-файлу.
#[test]
#[ignore = "requires official Unicode 17 Idna2008.txt through LITTERKEEP_IDNA2008_FILE"]
fn derived_property_matches_official_unicode_17_corpus() {
    let fixture = env::var_os("LITTERKEEP_IDNA2008_FILE")
        .expect("set LITTERKEEP_IDNA2008_FILE to Unicode 17 Idna2008.txt");
    let source = fs::read_to_string(&fixture).expect("read official IDNA2008 corpus");
    let mut covered_until = 0_u32;
    let mut scalar_count = 0_u32;

    for (line_number, line) in source.lines().enumerate() {
        let content = match line.split_once('#') {
            Some((content, _)) => content.trim(),
            None => line.trim(),
        };
        let record = match content.split_once(';') {
            Some((range, category)) => Some((range, category)),
            None if content.is_empty() => None,
            None => panic!("malformed IDNA2008 line {}: {line:?}", line_number + 1),
        };
        let Some((range, category)) = record else {
            continue;
        };
        let (start, end) = parse_range(range)
            .unwrap_or_else(|| panic!("invalid range on IDNA2008 line {}", line_number + 1));
        let expected = parse_category(category)
            .unwrap_or_else(|| panic!("unknown category on IDNA2008 line {}", line_number + 1));
        assert_eq!(
            start,
            covered_until,
            "corpus gap before line {}",
            line_number + 1
        );

        for code_point in start..=end {
            match char::from_u32(code_point) {
                Some(scalar) => {
                    assert_eq!(
                        derived_property(scalar),
                        expected,
                        "IDNA property mismatch at U+{code_point:04X}"
                    );
                    scalar_count += 1;
                }
                None => assert_eq!(expected, DerivedProperty::DisallowedOrUnassigned),
            }
        }
        covered_until = end + 1;
    }

    assert_eq!(
        covered_until, 0x110000,
        "corpus does not cover Unicode range"
    );
    assert_eq!(scalar_count, 0x110000 - 0x800, "unexpected scalar coverage");
}

/// Разбирает singleton либо замкнутый диапазон из Unicode property file.
fn parse_range(value: &str) -> Option<(u32, u32)> {
    let value = value.trim();
    match value.split_once("..") {
        Some((start, end)) => Some((parse_hex(start)?, parse_hex(end)?)),
        None => {
            let scalar = parse_hex(value)?;
            Some((scalar, scalar))
        }
    }
}

/// Преобразует одну hexadecimal code point запись без индексного доступа.
fn parse_hex(value: &str) -> Option<u32> {
    u32::from_str_radix(value.trim(), 16).ok()
}

/// Нормализует внешнее имя RFC 5892 категории в типизированное свойство.
fn parse_category(value: &str) -> Option<DerivedProperty> {
    match value.trim() {
        "PVALID" => Some(DerivedProperty::PValid),
        "CONTEXTJ" => Some(DerivedProperty::ContextJ),
        "CONTEXTO" => Some(DerivedProperty::ContextO),
        "DISALLOWED" | "UNASSIGNED" => Some(DerivedProperty::DisallowedOrUnassigned),
        _ => None,
    }
}
