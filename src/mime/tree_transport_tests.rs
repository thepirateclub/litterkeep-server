//! Сквозные проверки multipart ranges и их IMAP BODYSTRUCTURE-проекции.

use super::{MediaType, parse};

#[test]
fn padded_boundaries_preserve_part_ranges_and_bodystructure() {
    let raw = b"From: alice@test\r\nTo: bob@test\r\nSubject: padded\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\npreamble\r\n--x \t\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 7bit\r\n\r\nfirst line\r\nlast line\r\n--x\t\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\neA==\r\n--x-- \t\r\nepilogue";
    let entity = parse(raw).expect("multipart tree with transport padding");
    let first = entity.children.first().expect("first part");
    let second = entity.children.get(1).expect("second part");

    assert_eq!(entity.children.len(), 2);
    assert_eq!(first.media_type, MediaType::TextPlain);
    assert_eq!(
        raw.get(first.body_range.clone()),
        Some(b"first line\r\nlast line".as_slice())
    );
    assert_eq!(second.decoded_body(raw).expect("decoded attachment"), b"x");
    assert!(
        crate::mime::bodystructure::encode(&entity, raw)
            .expect("BODYSTRUCTURE")
            .ends_with(" \"MIXED\")")
    );
}
