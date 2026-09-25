//! Проверки Content-Language lists и Content-Location fields.

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
