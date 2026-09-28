//! Проверки UTF-8 quoted-string режима в IMAP MIME BODYSTRUCTURE.

use super::encode_extended_with_mode;
use crate::imap::tokenizer::StringMode;
use crate::mime::tree;

fn unicode_literal(value: &str) -> String {
    format!("{{{}}}\r\n{value}", value.len())
}

#[test]
fn legacy_uses_literals_and_enabled_mode_quotes_decoded_mime_strings() {
    let raw = "Content-Type: application/pdf; name*=UTF-8''caf%C3%A9.pdf\r\nContent-Disposition: attachment; filename*=UTF-8''caf%C3%A9.pdf\r\nContent-ID: <картинка@пример.рф>\r\nContent-Description: =?UTF-8?B?Y2Fmw6k=?=\r\nContent-Location: https://example.test/cafe%C3%A9\r\nContent-Transfer-Encoding: base64\r\n\r\nUERG".as_bytes();
    let entity = tree::parse(raw).expect("MIME entity");
    let legacy =
        encode_extended_with_mode(&entity, raw, StringMode::Legacy).expect("legacy BODYSTRUCTURE");
    let enabled = encode_extended_with_mode(&entity, raw, StringMode::Utf8Accept)
        .expect("UTF8=ACCEPT BODYSTRUCTURE");

    for value in ["café.pdf", "<картинка@пример.рф>", "café"] {
        assert!(legacy.contains(&unicode_literal(value)), "{legacy}");
        assert!(enabled.contains(&format!("\"{value}\"")), "{enabled}");
    }
}
