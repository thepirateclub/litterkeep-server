//! Минимальные MIME-примитивы для protocol и независимых проверок.
use std::collections::BTreeMap;

#[path = "mime/base64.rs"]
mod base64;
pub use base64::{decode_base64, decode_mime_base64};
#[path = "mime/bodystructure.rs"]
pub mod bodystructure;
#[path = "mime/charset.rs"]
pub mod charset;
#[path = "mime/charset_iso8859_2.rs"]
mod charset_iso8859_2;
#[path = "mime/charset_iso8859_3.rs"]
mod charset_iso8859_3;
#[path = "mime/charset_iso8859_4.rs"]
mod charset_iso8859_4;
#[path = "mime/charset_iso8859_5.rs"]
mod charset_iso8859_5;
#[path = "mime/charset_iso8859_6.rs"]
mod charset_iso8859_6;
#[path = "mime/content_id.rs"]
mod content_id;
#[path = "mime/content_metadata.rs"]
pub mod content_metadata;
#[path = "mime/disposition.rs"]
pub mod disposition;
#[path = "mime/encoded_word.rs"]
pub mod encoded_word;
#[path = "mime/envelope.rs"]
pub mod envelope;
#[path = "mime/envelope_address.rs"]
pub(crate) mod envelope_address;
#[path = "mime/envelope_address_scan.rs"]
mod envelope_address_scan;
#[path = "mime/headers.rs"]
pub mod header_block;
#[path = "mime/language_tag.rs"]
pub mod language_tag;
#[path = "mime/parameter_extended.rs"]
pub(crate) mod parameter_extended;
#[path = "mime/parameter_join.rs"]
mod parameter_join;
#[path = "mime/parameters.rs"]
pub mod parameters;
#[cfg(test)]
#[path = "mime/parameters_tests.rs"]
mod parameters_tests;
#[path = "mime/quoted_printable.rs"]
pub mod quoted_printable;
#[path = "mime/quoted_printable_encode.rs"]
pub(crate) mod quoted_printable_encode;
#[cfg(test)]
#[path = "mime/quoted_printable_rfc_tests.rs"]
mod quoted_printable_rfc_tests;
#[path = "mime/search_text.rs"]
pub mod search_text;
#[path = "mime/structured_comments.rs"]
pub(crate) mod structured_comments;
#[path = "mime/transfer.rs"]
pub mod transfer;
#[path = "mime/tree.rs"]
pub mod tree;
#[path = "mime/uri_reference.rs"]
pub(crate) mod uri_reference;
#[path = "mime/wsp.rs"]
pub(crate) mod wsp;

pub fn headers(raw: &[u8]) -> Result<(BTreeMap<String, String>, &[u8]), &'static str> {
    let parsed = header_block::parse(raw)?;
    let mut out = BTreeMap::new();
    for field in parsed.fields {
        let value = String::from_utf8(field.value).map_err(|_| "header value is not utf-8")?;
        out.insert(field.name, value);
    }
    Ok((
        out,
        raw.get(parsed.body_offset..)
            .ok_or("invalid body boundary")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_headers_and_attachment_bytes() {
        let raw = b"Subject: test\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8A";
        let (h, body) = headers(raw).expect("headers");
        assert_eq!(h.get("subject"), Some(&"test".to_owned()));
        assert_eq!(decode_base64(body).expect("base64"), b"hello\0");
    }
}
