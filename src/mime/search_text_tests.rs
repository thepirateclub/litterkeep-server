//! RFC 2047 и MIME transfer/charset acceptance для IMAP text search.

use super::*;

#[test]
fn decodes_encoded_words_and_nested_base64_and_quoted_printable_text() {
    let raw = b"Subject: =?UTF-8?B?0J/RgNC40LLQtdGC?=\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n0J/RgNC40LLQtdGC\r\n--x\r\nContent-Type: text/calendar; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\ncaf=C3=A9\r\n--x\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\n0J/RgNC40LLQtdGC\r\n--x--\r\n";
    let document = SearchDocument::parse(raw, true).expect("search document");
    assert!(document.body_contains("Привет"));
    assert!(document.body_contains("CAFé"));
    assert!(document.text_contains("Привет"));
    assert!(!document.body_contains("Subject only"));
    assert!(!document.body_contains("0J/RgNC40LLQtdGC"));
}

#[test]
fn header_only_document_decodes_rfc2047_without_scanning_the_body() {
    let raw = b"Subject: =?ISO-8859-1?Q?caf=E9?=\r\n\r\nbody secret";
    let document = SearchDocument::parse(raw, false).expect("headers");
    assert!(document.text_contains("café"));
    assert!(!document.body_contains("secret"));
}

#[test]
fn unknown_charset_and_malformed_transfer_are_not_searched_as_raw_octets() {
    let raw = b"Content-Type: text/plain; charset=windows-1251\r\nContent-Transfer-Encoding: base64\r\n\r\n0J/RgNC40LLQtdGC";
    let document = SearchDocument::parse(raw, true).expect("search document");
    assert!(!document.body_contains("Привет"));
    assert!(!document.body_contains("0J/RgNC40LLQtdGC"));
}
