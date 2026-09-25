//! Регрессии разницы базового BODY и расширенного BODYSTRUCTURE.

use super::{encode_basic, encode_extended};
use crate::mime::tree;

#[test]
fn basic_body_omits_extensions_that_extended_structure_returns() {
    let raw = b"Content-Type: application/pdf\r\nContent-Disposition: attachment\r\nContent-Language: en\r\nContent-Location: ./part.pdf\r\n\r\npdf";
    let entity = tree::parse(raw).expect("MIME entity");

    assert_eq!(
        encode_basic(&entity, raw).expect("BODY"),
        "(\"application\" \"pdf\" NIL NIL NIL \"7BIT\" 3)"
    );
    assert!(
        encode_extended(&entity, raw)
            .expect("BODYSTRUCTURE")
            .ends_with(" NIL (\"attachment\" NIL) \"en\" \"./part.pdf\")")
    );
}

#[test]
fn basic_body_includes_content_id_and_description_fields() {
    let raw = b"Content-Type: image/png\r\nContent-ID: <logo@example.test>\r\nContent-Description: logo\r\n\r\nPNG";
    let entity = tree::parse(raw).expect("MIME entity");

    assert_eq!(
        encode_basic(&entity, raw).expect("BODY"),
        "(\"image\" \"png\" NIL \"<logo@example.test>\" \"logo\" \"7BIT\" 3)"
    );
}
