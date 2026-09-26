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
fn rejects_invalid_disposition_tokens_and_parameters() {
    for raw in ["attachment extra", "attachment; filename=\"unfinished"] {
        assert!(parse(raw).is_err(), "accepted {raw}");
    }
}

#[test]
fn extension_dispositions_remain_available_without_string_classification() {
    let value = parse("x-company-review").expect("extension disposition");

    assert_eq!(
        value.kind,
        DispositionKind::Extension("x-company-review".to_owned())
    );
}

#[test]
fn rejects_unicode_whitespace_around_disposition_token() {
    assert!(parse("\u{00a0}attachment").is_err());
}
