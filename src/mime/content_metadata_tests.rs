//! Проверки стандартных MIME metadata-полей для IMAP BODYSTRUCTURE.

use super::{ContentMetadata, parse};
use crate::mime::header_block;

fn metadata(raw: &[u8]) -> ContentMetadata {
    let parsed = header_block::parse(raw).expect("header block");
    parse(&parsed.fields)
}

#[test]
fn parses_one_or_many_language_tags_and_comments() {
    let parsed = metadata(b"Content-Language: en-US, fr (comment), i-mingo\r\n\r\n");

    let languages = parsed.languages.expect("language list");
    assert_eq!(languages.len(), 3);
    assert_eq!(languages.first().map(|tag| tag.as_str()), Some("en-US"));
    assert_eq!(languages.get(1).map(|tag| tag.as_str()), Some("fr"));
    assert_eq!(languages.get(2).map(|tag| tag.as_str()), Some("i-mingo"));
}

#[test]
fn rejects_invalid_or_repeated_language_fields_as_one_unit() {
    for raw in [
        b"Content-Language: en--US\r\n\r\n".as_slice(),
        b"Content-Language: en\r\nContent-Language: fr\r\n\r\n",
        b"Content-Language: en,\r\n\r\n",
    ] {
        assert!(metadata(raw).languages.is_none());
    }
}

#[test]
fn preserves_relative_content_location_without_rewriting_uri_octets() {
    let parsed = metadata(b"Content-Location: images/logo%20small.svg\r\n\r\n");

    assert_eq!(parsed.location.as_deref(), Some("images/logo%20small.svg"));
}

#[test]
fn duplicate_or_empty_content_location_is_not_projected() {
    for raw in [
        b"Content-Location:\r\n\r\n".as_slice(),
        b"Content-Location: https://a.test/x\r\nContent-Location: https://a.test/y\r\n\r\n",
    ] {
        assert!(metadata(raw).location.is_none());
    }
}

#[test]
fn invalid_language_does_not_hide_valid_location() {
    let parsed = metadata(b"Content-Language: en--US\r\nContent-Location: ./part.html\r\n\r\n");

    assert!(parsed.languages.is_none());
    assert_eq!(parsed.location.as_deref(), Some("./part.html"));
}

#[test]
fn parses_content_id_and_rfc2047_description() {
    let parsed = metadata(
        b"Content-ID: <image.logo-1@example.test> (inline image)\r\nContent-Description: =?UTF-8?B?0J/RgNC40LLQtdGC?=\r\n\r\n",
    );

    assert_eq!(
        parsed.content_id.as_deref(),
        Some("<image.logo-1@example.test>")
    );
    assert_eq!(parsed.description.as_deref(), Some("Привет"));
}

#[test]
fn parses_quoted_domain_literal_and_utf8_content_ids() {
    assert_eq!(
        super::separator_at("\"image@logo\"@[IPv6:2001:db8::1]"),
        Some(12)
    );
    assert_eq!(
        super::normalize_id_left("\"image@logo\"").as_deref(),
        Some("\"image@logo\"")
    );
    assert_eq!(
        super::normalize_id_right("[IPv6:2001:db8::1]").as_deref(),
        Some("[IPv6:2001:db8::1]")
    );
    for (raw, expected) in [
        (
            b"Content-ID: <\"image@logo\"@[IPv6:2001:db8::1]>\r\n\r\n".as_slice(),
            "<\"image@logo\"@[IPv6:2001:db8::1]>",
        ),
        (
            "Content-ID: <картинка@пример.рф>\r\n\r\n".as_bytes(),
            "<картинка@пример.рф>",
        ),
    ] {
        assert_eq!(metadata(raw).content_id.as_deref(), Some(expected));
    }
}

/// Obsolete CFWS вокруг точек не входит в семантический Content-ID.
#[test]
fn normalizes_obsolete_cfws_in_content_id_sides() {
    for (header, expected) in [
        (
            b"Content-ID: <image (left) . logo @ example (right) . test>\r\n\r\n".as_slice(),
            "<image.logo@example.test>",
        ),
        (
            b"Content-ID: <image\t.\tlogo@example\t.\ttest>\r\n\r\n".as_slice(),
            "<image.logo@example.test>",
        ),
        (
            b"Content-ID: <image @ example.test>\r\n\r\n".as_slice(),
            "<image@example.test>",
        ),
    ] {
        assert_eq!(metadata(header).content_id.as_deref(), Some(expected));
    }
}

#[test]
fn omits_ambiguous_or_invalid_content_identity_fields() {
    for raw in [
        b"Content-ID: image@example.test\r\n\r\n".as_slice(),
        b"Content-ID: <image..logo@example.test>\r\n\r\n",
        b"Content-ID: <image@example.test@other.test>\r\n\r\n",
        b"Content-ID: <image@[broken]]>\r\n\r\n",
        b"Content-ID: <image@exa mple.test>\r\n\r\n",
        b"Content-ID: <image@example.test>\r\nContent-ID: <other@example.test>\r\n\r\n",
        b"Content-Description: first\r\nContent-Description: second\r\n\r\n",
    ] {
        let parsed = metadata(raw);
        assert!(parsed.content_id.is_none());
        assert!(parsed.description.is_none());
    }
}

#[test]
fn accepts_only_one_base64_encoded_sixteen_octet_content_md5() {
    let parsed = metadata(b"Content-MD5: XUFAKrxLKna5cZ2REBfFkg== (comment)\r\n\r\n");
    assert_eq!(
        parsed.content_md5.as_deref(),
        Some("XUFAKrxLKna5cZ2REBfFkg==")
    );

    for raw in [
        b"Content-MD5: YQ==\r\n\r\n".as_slice(),
        b"Content-MD5: XUFAKrxLKna5cZ2REBfFkg=!\r\n\r\n",
        b"Content-MD5: XUFAKrxLKna5cZ2REBfFkg==\r\nContent-MD5: XUFAKrxLKna5cZ2REBfFkg==\r\n\r\n",
    ] {
        assert!(metadata(raw).content_md5.is_none());
    }
}
