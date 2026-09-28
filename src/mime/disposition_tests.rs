//! Проверки RFC 2183 disposition и RFC 2231 filename для IMAP projection.

use super::{DispositionKind, encode_bodystructure, parse};

#[test]
fn parses_case_insensitive_standard_disposition_and_parameters() {
    let value = parse("Attachment; filename=\"report; final.pdf\"").expect("disposition");

    assert_eq!(value.kind, DispositionKind::Attachment);
    assert_eq!(
        value.parameters.get("filename").map(String::as_str),
        Some("report; final.pdf")
    );
}

#[test]
fn decodes_rfc2231_filename_before_bodystructure_encoding() {
    let value =
        parse("attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.pdf").expect("extended filename");

    assert_eq!(
        encode_bodystructure(&value).expect("IMAP disposition"),
        "(\"attachment\" (\"FILENAME\" \"résumé.pdf\"))"
    );
}

#[test]
fn preserves_rfc2231_language_hint_without_changing_bodystructure() {
    let value = parse("attachment; filename*=UTF-8'ru'%D1%84%D0%B0%D0%B9%D0%BB.txt")
        .expect("language-tagged extended filename");

    assert_eq!(
        value
            .parameter_languages
            .get("filename")
            .map(|tag| tag.as_str()),
        Some("ru")
    );
    assert_eq!(
        value.parameters.get("filename").map(String::as_str),
        Some("файл.txt")
    );
    assert_eq!(
        encode_bodystructure(&value).expect("IMAP disposition"),
        "(\"attachment\" (\"FILENAME\" \"файл.txt\"))"
    );
}

#[test]
fn rejects_invalid_disposition_tokens_and_parameters() {
    for raw in ["attachment extra", "attachment; filename=\"unfinished"] {
        assert!(parse(raw).is_err(), "accepted {raw}");
    }
}

/// RFC 2183 §2.8 требует сохранять известные параметры при чужом charset.
#[test]
fn ignores_one_unrecognized_extended_parameter() {
    let value =
        parse("attachment; filename*=x-litterkeep-unknown''secret.bin; size=12; x-vendor=ok")
            .expect("unknown parameter is ignored independently");

    assert!(!value.parameters.contains_key("filename"));
    assert_eq!(value.parameters.get("size").map(String::as_str), Some("12"));
    assert_eq!(
        value.parameters.get("x-vendor").map(String::as_str),
        Some("ok")
    );
}

/// Повтор known parameter не выбирает произвольно первое или последнее имя.
#[test]
fn ignores_ambiguous_repeated_parameter_without_losing_other_values() {
    let value = parse("attachment; filename=one.txt; filename=two.txt; size=12")
        .expect("unknown repeated parameter does not poison disposition");

    assert!(!value.parameters.contains_key("filename"));
    assert_eq!(value.parameters.get("size").map(String::as_str), Some("12"));
}

/// Неизвестное disposition по RFC 2183 §2.8 безопасно трактуется как attachment.
#[test]
fn extension_dispositions_fall_back_to_attachment() {
    let value = parse("x-company-review").expect("extension disposition");

    assert_eq!(value.kind, DispositionKind::Attachment);
}

/// Параметры остаются применимы и когда disposition type неизвестен.
#[test]
fn extension_disposition_keeps_known_filename_parameter() {
    let value = parse("x-company-review; filename=report.pdf").expect("extension disposition");

    assert_eq!(value.kind, DispositionKind::Attachment);
    assert_eq!(
        value.parameters.get("filename").map(String::as_str),
        Some("report.pdf")
    );
}

/// Некорректный size пропускается отдельно, не теряя соседнее имя файла.
#[test]
fn ignores_size_parameter_with_non_decimal_octets() {
    let value = parse("attachment; filename=report.pdf; size=12x").expect("disposition");

    assert_eq!(
        value.parameters.get("filename").map(String::as_str),
        Some("report.pdf")
    );
    assert!(!value.parameters.contains_key("size"));
}

/// RFC 2183 допускает ненулевую последовательность десятичных цифр в size.
#[test]
fn preserves_decimal_size_parameter() {
    let value = parse("attachment; size=0012").expect("disposition");

    assert_eq!(
        value.parameters.get("size").map(String::as_str),
        Some("0012")
    );
}

/// Размер в quoted-string или RFC 2231 форме не является RFC 2183 `size`.
#[test]
fn rejects_quoted_and_extended_size_parameter_forms() {
    for parameter in ["size=\"12\"", "size*=UTF-8''12"] {
        let value = parse(&format!("attachment; filename=report.pdf; {parameter}"))
            .expect("invalid size does not poison disposition");

        assert!(
            !value.parameters.contains_key("size"),
            "accepted {parameter}"
        );
        assert_eq!(
            value.parameters.get("filename").map(String::as_str),
            Some("report.pdf")
        );
    }
}

#[test]
fn rejects_unicode_whitespace_around_disposition_token() {
    assert!(parse("\u{00a0}attachment").is_err());
}
