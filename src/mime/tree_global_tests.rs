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

/// Base64 source сохраняет собственные диапазоны и вложенную ENVELOPE.
#[test]
fn parses_base64_global_message_with_owned_decoded_source() {
    let raw = b"Content-Type: message/global\r\nContent-Transfer-Encoding: base64\r\n\r\nU3ViamVjdDogbmVzdGVkDQpDb250ZW50LVR5cGU6IHRleHQvcGxhaW4NCg0KYm9keQ==";
    let decoded = b"Subject: nested\r\nContent-Type: text/plain\r\n\r\nbody";
    let tree = parse(raw).expect("base64 message/global");
    let nested = tree.children.first().expect("decoded embedded message");

    assert_eq!(nested.raw_bytes(raw), Ok(decoded.as_slice()));
    let structure = crate::mime::bodystructure::encode_extended(&tree, raw)
        .expect("encoded global BODYSTRUCTURE");
    assert!(structure.starts_with("(\"MESSAGE\" \"GLOBAL\" NIL NIL NIL \"BASE64\""));
    assert!(structure.contains("(NIL \"nested\" NIL NIL NIL NIL NIL NIL NIL NIL)"));

    let encoded_body = crate::imap_fetch::select(
        raw,
        &crate::imap::FetchAttribute::Body {
            peek: true,
            section: crate::imap::BodySection::Part {
                path: vec![1],
                mime_headers: false,
            },
            partial: None,
        },
    )
    .expect("raw message/global body");
    assert_eq!(
        encoded_body.bytes.as_ref(),
        b"U3ViamVjdDogbmVzdGVkDQpDb250ZW50LVR5cGU6IHRleHQvcGxhaW4NCg0KYm9keQ=="
    );

    let nested_body = crate::imap_fetch::select(
        raw,
        &crate::imap::FetchAttribute::Body {
            peek: true,
            section: crate::imap::BodySection::Part {
                path: vec![1, 1],
                mime_headers: false,
            },
            partial: None,
        },
    )
    .expect("decoded nested part body");
    assert_eq!(nested_body.label, "BODY[1.1]");
    assert_eq!(nested_body.bytes.as_ref(), b"body");

    let headers = crate::imap_fetch::select(
        raw,
        &crate::imap::FetchAttribute::Body {
            peek: true,
            section: crate::imap::BodySection::Headers { path: vec![1] },
            partial: None,
        },
    )
    .expect("decoded global headers");
    assert_eq!(
        headers.bytes.as_ref(),
        b"Subject: nested\r\nContent-Type: text/plain\r\n\r\n"
    );
}

/// Quoted-printable message/global decodes a MIME header before tree parsing.
#[test]
fn parses_quoted_printable_global_message() {
    let raw = b"Content-Type: message/global\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nSubject=3A nested\r\nContent-Type: text/plain\r\n\r\nbody";
    let tree = parse(raw).expect("quoted-printable message/global");
    let nested = tree.children.first().expect("decoded embedded message");

    assert_eq!(
        nested.headers.first().map(|field| field.name.as_str()),
        Some("subject")
    );
    assert!(
        crate::mime::bodystructure::encode_extended(&tree, raw)
            .expect("quoted-printable BODYSTRUCTURE")
            .contains("(NIL \"nested\" NIL NIL NIL NIL NIL NIL NIL NIL)")
    );
}

/// Неизвестный CTE остаётся непрозрачным octet-stream, не ложным сообщением.
#[test]
fn unknown_global_transfer_encoding_uses_opaque_octet_stream_fallback() {
    let raw = b"Content-Type: message/global\r\nContent-Transfer-Encoding: x-future\r\n\r\nopaque";
    let tree = parse(raw).expect("unknown CTE remains opaque");

    assert_eq!(tree.media_type, super::MediaType::ApplicationOctetStream);
    assert!(tree.children.is_empty());
}
