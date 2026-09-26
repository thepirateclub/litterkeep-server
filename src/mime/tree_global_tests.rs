//! Проверки вложенных UTF-8 сообщений типа message/global.

use super::{MediaType, parse};

/// RFC 6532 message/global сохраняет исходные диапазоны вложенного сообщения.
#[test]
fn parses_identity_encoded_global_message_with_utf8_headers() {
    let raw = "Content-Type: message/global\r\nContent-Transfer-Encoding: 8bit\r\n\r\nSubject: Привет\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\nТело\r\n".as_bytes();
    let tree = parse(raw).expect("identity-encoded message/global");
    let nested = tree.children.first().expect("encapsulated message");

    assert_eq!(tree.media_type, MediaType::MessageGlobal);
    assert_eq!(nested.media_type, MediaType::TextPlain);
    assert_eq!(
        crate::mime::bodystructure::encode_extended(&tree, raw).expect("BODYSTRUCTURE"),
        "(\"MESSAGE\" \"GLOBAL\" NIL NIL NIL \"8BIT\" 109 (NIL \"Привет\" NIL NIL NIL NIL NIL NIL NIL NIL) (\"TEXT\" \"PLAIN\" (\"CHARSET\" \"utf-8\") NIL NIL \"8BIT\" 10 1) 5)"
    );
}

/// IMAP HEADER section может выбирать заголовки вложенного message/global.
#[test]
fn imap_nested_header_selector_accepts_global_message_path() {
    let raw = b"Content-Type: message/global\r\n\r\nSubject: nested\r\n\r\nbody";
    let selected = crate::imap_fetch::select(
        raw,
        &crate::imap::FetchAttribute::Body {
            peek: true,
            section: crate::imap::BodySection::Headers { path: vec![1] },
            partial: None,
        },
    )
    .expect("nested global headers");

    assert_eq!(selected.label, "BODY[1.HEADER]");
    assert_eq!(selected.bytes.as_ref(), b"Subject: nested\r\n\r\n");
}

/// Недоступный decoded-range mapping не позволяет принять ложную структуру.
#[test]
fn rejects_non_identity_global_transfer_encoding_until_ranges_are_mapped() {
    let raw = b"Content-Type: message/global\r\nContent-Transfer-Encoding: base64\r\n\r\nU3ViamVjdDogbmVzdGVkDQoNCmJvZHk=";

    assert!(parse(raw).is_err());
}
