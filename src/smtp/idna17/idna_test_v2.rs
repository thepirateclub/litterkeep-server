//! Сверка strict IDNA2008 для общего валидного подмножества UTS #46 corpus.

use super::{DerivedProperty, derived_property, to_alabel_domain};
use std::{env, fs};

/// Сравнивает ASCII form только для NFC/PVALID строк без UTS #46 ошибок.
///
/// UTS #46 содержит отображения и transitional-правила, которых нет в
/// IDNA2008. Поэтому данный oracle намеренно проверяет лишь пересечение:
/// NFC, только PVALID scalar values, корректную непустую структуру домена и
/// пустой набор статусов nontransitional ToASCII Unicode 17.
#[test]
#[ignore = "requires Unicode 17 IdnaTestV2.txt through LITTERKEEP_IDNA_TEST_V2_FILE"]
fn strict_idna_matches_valid_nfc_pvalid_uts46_rows() {
    let fixture = env::var_os("LITTERKEEP_IDNA_TEST_V2_FILE")
        .expect("set LITTERKEEP_IDNA_TEST_V2_FILE to Unicode 17 IdnaTestV2.txt");
    let source = fs::read_to_string(&fixture).expect("read official Unicode IDNA test corpus");
    let mut checked = 0_usize;
    let mut skipped_non_scalars = 0_usize;

    for (line_number, line) in source.lines().enumerate() {
        match parse_row(line, line_number + 1) {
            ParsedRow::Candidate(row) if is_strict_overlap(&row) => {
                assert_eq!(
                    to_alabel_domain(&row.source),
                    Ok(row.expected_ascii),
                    "strict IDNA mismatch at IdnaTestV2 line {}",
                    line_number + 1
                );
                checked = checked.saturating_add(1);
            }
            ParsedRow::Candidate(_) | ParsedRow::Ignored => {}
            ParsedRow::NonScalar => skipped_non_scalars = skipped_non_scalars.saturating_add(1),
        }
    }

    assert!(checked >= 100, "too few overlap rows checked: {checked}");
    eprintln!(
        "strict IDNA / UTS #46 overlap rows checked: {checked}; non-scalar rows skipped: {skipped_non_scalars}"
    );
}

/// Выделяет минимальные поля строки, не интерпретируя комментарий как данные.
fn parse_row(line: &str, line_number: usize) -> ParsedRow {
    let content = match line.split_once('#') {
        Some((data, _)) => data.trim(),
        None => line.trim(),
    };
    if content.is_empty() {
        return ParsedRow::Ignored;
    }
    let fields = content.split(';').map(str::trim).collect::<Vec<_>>();
    if fields.len() < 7 {
        return malformed_row(line_number, "fewer than seven columns");
    }
    let source_field = fields
        .first()
        .copied()
        .unwrap_or_else(|| malformed_row(line_number, "missing source column"));
    let unicode_field = fields
        .get(1)
        .copied()
        .unwrap_or_else(|| malformed_row(line_number, "missing toUnicode column"));
    let unicode_status_field = fields
        .get(2)
        .copied()
        .unwrap_or_else(|| malformed_row(line_number, "missing toUnicode status column"));
    let ascii_field = fields
        .get(3)
        .copied()
        .unwrap_or_else(|| malformed_row(line_number, "missing toAsciiN column"));
    let ascii_status_field = fields
        .get(4)
        .copied()
        .unwrap_or_else(|| malformed_row(line_number, "missing toAsciiN status column"));
    let [decoded_source, decoded_unicode, decoded_ascii] =
        match decode_corpus_fields(source_field, unicode_field, ascii_field) {
            Ok(fields) => fields,
            Err(DecodeError::NonScalar) => return ParsedRow::NonScalar,
            Err(DecodeError::Malformed) => malformed_row(line_number, "invalid escape sequence"),
        };
    let unicode_status = effective_value(unicode_status_field, "");
    let ascii_status = effective_value(ascii_status_field, unicode_status);
    let expected_unicode = resolve_column(unicode_field, &decoded_unicode, &decoded_source);
    let expected_ascii = resolve_column(ascii_field, &decoded_ascii, expected_unicode).to_owned();
    let unicode_result = expected_unicode.to_owned();
    ParsedRow::Candidate(TestRow {
        source: decoded_source,
        unicode_result,
        expected_ascii,
        unicode_status: parse_status(unicode_status),
        ascii_status: parse_status(ascii_status),
    })
}

/// Останавливает corpus oracle на malformed записи с точной позицией.
fn malformed_row<T>(line_number: usize, reason: &str) -> T {
    panic!("malformed IdnaTestV2 line {line_number}: {reason}");
}

/// Преобразует пустое поле-алиас или явное обозначение пустой строки.
fn effective_value<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    match value {
        "" => fallback,
        "\"\"" => "",
        value => value,
    }
}

/// Разрешает правила формата: пустое поле наследует предыдущий результат.
fn resolve_column<'a>(field: &str, decoded: &'a str, fallback: &'a str) -> &'a str {
    match field {
        "" => fallback,
        "\"\"" => "",
        _ => decoded,
    }
}

/// Сводит форматный список статусов к исходу, который нужен строгому oracle.
fn parse_status(value: &str) -> StatusState {
    match value {
        "" | "[]" => StatusState::NoErrors,
        _ => StatusState::HasErrors,
    }
}

/// Декодирует три поля, изолируя surrogate rows от malformed escape синтаксиса.
fn decode_corpus_fields(
    source: &str,
    unicode: &str,
    ascii: &str,
) -> Result<[String; 3], DecodeError> {
    Ok([
        decode_field(source)?,
        decode_field(unicode)?,
        decode_field(ascii)?,
    ])
}

/// Декодирует поле Unicode corpus, сохраняя строгую проверку escape синтаксиса.
fn decode_field(value: &str) -> Result<String, DecodeError> {
    if value == "\"\"" {
        return Ok(String::new());
    }
    let mut output = String::new();
    let mut input = value.chars().peekable();
    while let Some(character) = input.next() {
        match character {
            '\\' => decode_escape(&mut input, &mut output)?,
            character => output.push(character),
        }
    }
    Ok(output)
}

/// Декодирует `\uXXXX` и `\x{XXXX}` без индексного доступа к строкам.
fn decode_escape(
    input: &mut std::iter::Peekable<std::str::Chars<'_>>,
    output: &mut String,
) -> Result<(), DecodeError> {
    let kind = input.next().ok_or(DecodeError::Malformed)?;
    let digits = match kind {
        'u' => take_hex_digits(input, 4)?,
        'x' if input.next() == Some('{') => take_braced_hex(input)?,
        _ => return Err(DecodeError::Malformed),
    };
    let scalar = u32::from_str_radix(&digits, 16).map_err(|_| DecodeError::Malformed)?;
    output.push(char::from_u32(scalar).ok_or(DecodeError::NonScalar)?);
    Ok(())
}

/// Собирает ровно указанное количество hexadecimal digits.
fn take_hex_digits(
    input: &mut std::iter::Peekable<std::str::Chars<'_>>,
    count: usize,
) -> Result<String, DecodeError> {
    let mut digits = String::new();
    for _ in 0..count {
        digits.push(input.next().ok_or(DecodeError::Malformed)?);
    }
    match digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        true => Ok(digits),
        false => Err(DecodeError::Malformed),
    }
}

/// Собирает произвольное hexadecimal scalar значение до закрывающей скобки.
fn take_braced_hex(
    input: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> Result<String, DecodeError> {
    let mut digits = String::new();
    loop {
        match input.next().ok_or(DecodeError::Malformed)? {
            '}' => {
                return match digits.is_empty()
                    || !digits.chars().all(|digit| digit.is_ascii_hexdigit())
                {
                    true => Err(DecodeError::Malformed),
                    false => Ok(digits),
                };
            }
            digit => digits.push(digit),
        }
    }
}

/// Проверяет принадлежность строки только к обещанному strict-пересечению.
fn is_strict_overlap(row: &TestRow) -> bool {
    matches!(
        (
            row.unicode_status == StatusState::NoErrors,
            row.ascii_status == StatusState::NoErrors,
            crate::unicode::nfc::normalize(&row.source) == row.source,
            source_has_only_strict_pvalid_labels(row),
        ),
        (true, true, true, true)
    )
}

/// Проверяет сырые U-label и декодированные A-label по RFC 5892 PVALID.
fn source_has_only_strict_pvalid_labels(row: &TestRow) -> bool {
    let mut source_labels = row.source.split('.');
    let mut unicode_labels = row.unicode_result.split('.');
    loop {
        match (source_labels.next(), unicode_labels.next()) {
            (Some(source), Some(unicode)) => {
                let property_label = match is_alabel(source) {
                    true => unicode,
                    false => source,
                };
                match (
                    property_label.is_empty(),
                    label_contains_only_pvalid(property_label),
                ) {
                    (false, true) => {}
                    _ => return false,
                }
            }
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Распознаёт A-label, чтобы проверять decoded U-label, а не ASCII префикс.
fn is_alabel(label: &str) -> bool {
    label
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("xn--"))
}

/// Не допускает CONTEXT и запрещённые scalar values в overlap-кorpus.
fn label_contains_only_pvalid(label: &str) -> bool {
    label
        .chars()
        .all(|character| derived_property(character) == DerivedProperty::PValid)
}

/// Одна декодированная строка и ожидаемая nontransitional ASCII-форма.
struct TestRow {
    source: String,
    unicode_result: String,
    expected_ascii: String,
    unicode_status: StatusState,
    ascii_status: StatusState,
}

/// Результат чтения записи corpus: комментарий, non-scalar либо кандидат.
enum ParsedRow {
    Ignored,
    NonScalar,
    Candidate(TestRow),
}

/// Ошибка fixture: некорректный escape либо scalar вне Unicode диапазона.
#[derive(Clone, Copy)]
enum DecodeError {
    Malformed,
    NonScalar,
}

/// Нормализованный исход UTS #46 проверки ToASCII nontransitional.
#[derive(Clone, Copy, Eq, PartialEq)]
enum StatusState {
    NoErrors,
    HasErrors,
}
