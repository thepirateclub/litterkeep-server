//! Минимальные MIME-примитивы для protocol и независимых проверок.
use std::collections::BTreeMap;

#[path = "mime/base64.rs"]
mod base64;
pub use base64::{decode_base64, decode_mime_base64};
#[path = "mime/bodystructure.rs"]
pub mod bodystructure;
#[path = "mime/charset.rs"]
pub mod charset;
#[path = "mime/charset_big5.rs"]
mod charset_big5;
#[path = "mime/charset_big5_table.rs"]
mod charset_big5_table;
#[path = "mime/charset_euc_jp.rs"]
mod charset_euc_jp;
#[path = "mime/charset_euc_jp_table.rs"]
mod charset_euc_jp_table;
#[path = "mime/charset_euc_kr.rs"]
mod charset_euc_kr;
#[path = "mime/charset_euc_kr_table.rs"]
mod charset_euc_kr_table;
#[cfg(test)]
#[path = "mime/charset_euc_kr_table_exhaustive_tests.rs"]
mod charset_euc_kr_table_exhaustive_tests;
#[path = "mime/charset_gb18030.rs"]
mod charset_gb18030;
#[path = "mime/charset_gb18030_table.rs"]
mod charset_gb18030_table;
#[path = "mime/charset_ibm866.rs"]
mod charset_ibm866;
#[path = "mime/charset_iso2022_jp.rs"]
mod charset_iso2022_jp;
#[path = "mime/charset_iso8859_10.rs"]
mod charset_iso8859_10;
#[path = "mime/charset_iso8859_13.rs"]
mod charset_iso8859_13;
#[path = "mime/charset_iso8859_14.rs"]
mod charset_iso8859_14;
#[path = "mime/charset_iso8859_15.rs"]
mod charset_iso8859_15;
#[path = "mime/charset_iso8859_16.rs"]
mod charset_iso8859_16;
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
#[path = "mime/charset_iso8859_7.rs"]
mod charset_iso8859_7;
#[path = "mime/charset_iso8859_8.rs"]
mod charset_iso8859_8;
#[path = "mime/charset_iso8859_9.rs"]
mod charset_iso8859_9;
#[path = "mime/charset_koi8_r.rs"]
mod charset_koi8_r;
#[path = "mime/charset_koi8_u.rs"]
mod charset_koi8_u;
#[path = "mime/charset_shift_jis.rs"]
mod charset_shift_jis;
#[path = "mime/charset_tis620.rs"]
mod charset_tis620;
#[path = "mime/charset_utf16.rs"]
mod charset_utf16;
#[path = "mime/charset_utf32.rs"]
mod charset_utf32;
#[cfg(test)]
#[path = "mime/charset_utf32_integration_tests.rs"]
mod charset_utf32_integration_tests;
#[path = "mime/charset_windows1250.rs"]
mod charset_windows1250;
#[path = "mime/charset_windows1251_1252.rs"]
mod charset_windows1251_1252;
#[path = "mime/charset_windows1253.rs"]
mod charset_windows1253;
#[path = "mime/charset_windows1254.rs"]
mod charset_windows1254;
#[path = "mime/charset_windows1255.rs"]
mod charset_windows1255;
#[path = "mime/charset_windows1256.rs"]
mod charset_windows1256;
#[path = "mime/charset_windows1257.rs"]
mod charset_windows1257;
#[path = "mime/charset_windows1258.rs"]
mod charset_windows1258;
#[path = "mime/charset_windows874.rs"]
mod charset_windows874;
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
#[path = "mime/envelope_address_comments.rs"]
mod envelope_address_comments;
#[path = "mime/envelope_address_dtext.rs"]
mod envelope_address_dtext;
#[path = "mime/envelope_address_route.rs"]
mod envelope_address_route;
#[path = "mime/envelope_address_scan.rs"]
mod envelope_address_scan;
#[path = "mime/headers.rs"]
pub mod header_block;
#[path = "mime/imap_string.rs"]
mod imap_string;
#[path = "mime/language_tag.rs"]
pub mod language_tag;
#[cfg(test)]
#[path = "mime/parameter_case_tests.rs"]
mod parameter_case_tests;
#[cfg(test)]
#[path = "mime/parameter_conflict_tests.rs"]
mod parameter_conflict_tests;
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
