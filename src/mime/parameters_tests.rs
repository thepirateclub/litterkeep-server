//! Регрессионные проверки MIME structured value и параметров.

use super::parameters::{self, MediaType};

#[test]
fn parses_quoted_boundary_and_filename_continuation() {
    let value = "application/octet-stream; boundary=\"a;b\"; filename*0*=UTF-8''%D1%81%D1%87; filename*1*=%D1%91%D1%82.txt";
    let parsed = parameters::parse(value).expect("structured MIME value");
    assert_eq!(parsed.media_type, MediaType::ApplicationOctetStream);
    assert_eq!(
        parsed.parameters.get("boundary").map(String::as_str),
        Some("a;b")
    );
    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("счёт.txt")
    );
}

#[test]
fn rejects_gapped_continuation_and_unknown_charset() {
    assert!(parameters::parse("text/plain; name*1=x").is_err());
    assert!(parameters::parse("text/plain; name*=koi8-r''text").is_err());
}

#[test]
fn parses_nested_comments_around_media_type_tokens() {
    let parsed = parameters::parse("multipart (outer(inner))/related; boundary=\"x(y)\"")
        .expect("comments around MIME tokens");

    assert_eq!(parsed.media_type, MediaType::MultipartRelated);
    assert_eq!(
        parsed.parameters.get("boundary").map(String::as_str),
        Some("x(y)")
    );
}

#[test]
fn rejects_invalid_media_type_tokens() {
    for value in [
        "text",
        "/plain",
        "text/",
        "text/plain/extra",
        "text/pl ain",
        "text/pl?ain",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value}");
    }
}

#[test]
fn retains_well_formed_unknown_media_type() {
    let parsed = parameters::parse("image/vnd.example+json").expect("valid extension type");

    assert_eq!(
        parsed.media_type,
        MediaType::Other("image/vnd.example+json".to_owned())
    );
}

#[test]
fn enforces_parameter_attribute_and_unquoted_value_grammar() {
    for value in [
        "text/plain; bad name=x",
        "text/plain; bad%name=x",
        "text/plain; filename*01=x",
        "text/plain; filename*foo=x",
        "text/plain; name=two words",
        "text/plain; name=bad?value",
        "text/plain; name=bad\\\"quote",
        "text/plain; name=",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value}");
    }
}

#[test]
fn accepts_empty_and_escaped_quoted_parameter_values() {
    let parsed = parameters::parse(r#"application/octet-stream; empty=""; name="x\"y/()""#)
        .expect("quoted MIME parameter values");

    assert_eq!(parsed.parameters.get("empty").map(String::as_str), Some(""));
    assert_eq!(
        parsed.parameters.get("name").map(String::as_str),
        Some("x\"y/()")
    );
    assert!(parameters::parse(r#"text/plain; name="x"y""#).is_err());
}

#[test]
fn accepts_mixed_rfc2231_continuations_and_encoded_language_value() {
    let parsed = parameters::parse(
        r#"application/octet-stream; url*0="ftp://"; url*1="example.test/a"; filename*=UTF-8'en'na%C3%AFve.txt"#,
    )
    .expect("mixed continuation forms");

    assert_eq!(
        parsed.parameters.get("url").map(String::as_str),
        Some("ftp://example.test/a")
    );
    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("naïve.txt")
    );
}

#[test]
/// Проверяет RFC 2231 filename* с байтами зарегистрированной IBM866 charset.
fn decodes_ibm866_extended_filename_parameter() {
    let parsed =
        parameters::parse("application/octet-stream; filename*=IBM866''%8F%E0%A8%A2%A5%E2.txt")
            .expect("IBM866 extended filename");

    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("Привет.txt")
    );
}

#[test]
fn decodes_windows_1252_rfc2231_filename() {
    let parsed = parameters::parse("application/octet-stream; filename*=windows-1252''%80%96")
        .expect("registered Windows-1252 filename");

    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("€–")
    );
}

#[test]
fn decodes_utf16be_rfc2231_filename() {
    let parsed = parameters::parse("application/octet-stream; filename*=UTF-16BE''%00H%00i")
        .expect("registered UTF-16BE filename");

    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("Hi")
    );
}

#[test]
fn preserves_rfc2231_language_metadata_for_single_and_continued_values() {
    let single = parameters::parse(
        "application/octet-stream; filename*=UTF-8'en-US'%D1%84%D0%B0%D0%B9%D0%BB.txt",
    )
    .expect("single extended parameter with language");
    assert_eq!(
        single
            .parameter_languages
            .get("filename")
            .map(|tag| tag.as_str()),
        Some("en-US")
    );

    let continued = parameters::parse(
        "application/octet-stream; filename*0*=UTF-8'ru'%D1%84%D0%B0; filename*1*=%D0%B9%D0%BB.txt",
    )
    .expect("continued extended parameter with language");
    assert_eq!(
        continued.parameters.get("filename").map(String::as_str),
        Some("файл.txt")
    );
    assert_eq!(
        continued
            .parameter_languages
            .get("filename")
            .map(|tag| tag.as_str()),
        Some("ru")
    );
}

#[test]
fn accepts_omitted_rfc2231_charset_only_for_ascii_payloads() {
    let parsed = parameters::parse("application/octet-stream; filename*='en'file%20name")
        .expect("ASCII payload may omit RFC 2231 charset");
    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("file name")
    );
    assert_eq!(
        parsed
            .parameter_languages
            .get("filename")
            .map(|tag| tag.as_str()),
        Some("en")
    );

    let continued =
        parameters::parse("application/octet-stream; filename*0*=''file%20; filename*1=name")
            .expect("ASCII continuation may omit RFC 2231 charset");
    assert_eq!(
        continued.parameters.get("filename").map(String::as_str),
        Some("file name")
    );
    assert!(parameters::parse("application/octet-stream; filename*='ru'%D1%84").is_err());
}

#[test]
fn rejects_malformed_rfc2231_language_tags_and_keeps_mime_tree_metadata() {
    for tag in ["en--US", "en-u", "x", "русский"] {
        let value = format!("application/octet-stream; filename*=UTF-8'{tag}'value");
        assert!(parameters::parse(&value).is_err(), "accepted {tag}");
    }

    let raw = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: application/octet-stream; filename*=UTF-8'en-GB-oed'value\r\n\r\nbody\r\n--x--\r\n";
    let entity = super::tree::parse(raw).expect("MIME tree keeps language metadata");
    let attachment = entity.children.first().expect("MIME attachment child");
    assert_eq!(
        attachment
            .parameter_languages
            .get("filename")
            .map(|tag| tag.as_str()),
        Some("en-GB-oed")
    );
}

#[test]
fn rejects_noncanonical_rfc2231_extended_payload_octets() {
    for value in [
        "text/plain; name*=UTF-8''bad?value",
        "text/plain; name*=UTF-8''bad'quote",
        "text/plain; name*=UTF-8''bad*value",
        "text/plain; name*=UTF-8''bad%G0value",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value}");
    }
}

/// ABNF HEXDIG нечувствителен к регистру и принимает lowercase escapes.
#[test]
fn decodes_lowercase_and_mixed_case_rfc2231_percent_escapes() {
    let parsed =
        parameters::parse("application/octet-stream; filename*=UTF-8''%d1%81%D1%87%D1%91.txt")
            .expect("case-insensitive hexadecimal escapes");

    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("счё.txt")
    );
}

/// RFC 6532 разрешает UTF-8 внутри quoted-string значения MIME-параметра.
#[test]
fn preserves_utf8_quoted_mime_parameter_value() {
    let parsed = parameters::parse("application/octet-stream; name=\"архив-✓.bin\"")
        .expect("UTF-8 quoted MIME parameter");

    assert_eq!(
        parsed.parameters.get("name").map(String::as_str),
        Some("архив-✓.bin")
    );
}

#[test]
fn rejects_unterminated_comment_but_preserves_quoted_parentheses() {
    assert!(parameters::parse("text/plain (unfinished").is_err());
    let parsed = parameters::parse("application/octet-stream; name=\"x(y)\"")
        .expect("parentheses inside quoted parameter");

    assert_eq!(
        parsed.parameters.get("name").map(String::as_str),
        Some("x(y)")
    );
}

/// RFC 5322 WSP содержит только SP и HTAB, не все Unicode-пробелы.
#[test]
fn rejects_unicode_whitespace_outside_mime_parameter_values() {
    for value in [
        "\u{00a0}text/plain",
        "text/plain;\u{00a0}name=value",
        "text/plain; name=\u{00a0}value",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value:?}");
    }
}

#[test]
fn trims_ascii_wsp_but_preserves_unicode_whitespace_inside_quotes() {
    let parsed = parameters::parse(" \ttext/plain\t; \tname\t=\t\"\u{00a0}\" \t")
        .expect("RFC WSP and quoted Unicode value");

    assert_eq!(
        parsed.parameters.get("name").map(String::as_str),
        Some("\u{00a0}")
    );
}
