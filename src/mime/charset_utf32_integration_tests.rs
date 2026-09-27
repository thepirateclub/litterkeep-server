//! Проверка интеграции UTF-32BE с MIME structured parameter parser.

use crate::mime::parameters;

#[test]
/// RFC 2231 filename* декодирует percent-octets UTF-32BE в Unicode scalar.
fn decodes_utf32be_extended_filename_parameter() {
    let parsed = parameters::parse("application/octet-stream; filename*=UTF-32BE''%00%01%F6%00")
        .expect("UTF-32BE extended filename");

    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("😀")
    );
}
