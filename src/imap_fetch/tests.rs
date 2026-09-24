//! Проверки извлечения исходных BODY sections и частичных октетных диапазонов.

use super::*;
use crate::imap::BodySection;

/// BODY.PEEK сохраняет байты части и применяет partial после выбора section.
#[test]
fn selects_encoded_part_and_octet_partial() {
    let message = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nabcdef\r\n--x--\r\n";
    let attribute = FetchAttribute::Body {
        peek: true,
        section: BodySection::Part {
            path: vec![1],
            mime_headers: false,
        },
        partial: Some(Partial {
            offset: 2,
            count: 3,
        }),
    };
    let selected = select(message, &attribute).expect("section");
    assert_eq!(selected.label, "BODY[1]<2>");
    assert_eq!(selected.bytes.as_ref(), b"cde");
}

/// HEADER включает delimiting blank line, а TEXT начинается после него.
#[test]
fn selects_message_header_and_body_sections() {
    let message = b"Subject: report\r\nX-Trace: id\r\n\r\nbody bytes";
    let headers =
        select(message, &body(BodySection::Headers { path: Vec::new() })).expect("headers");
    let text = select(message, &body(BodySection::Text { path: Vec::new() })).expect("text");
    assert_eq!(
        headers.bytes.as_ref(),
        b"Subject: report\r\nX-Trace: id\r\n\r\n"
    );
    assert_eq!(text.bytes.as_ref(), b"body bytes");
}

/// HEADER.FIELDS отбирает повторы без потери исходного порядка и folded lines.
#[test]
fn selects_header_fields_case_insensitively_and_keeps_repeats() {
    let message = b"Subject: first\r\nX-Trace: one\r\n\tcontinued\r\nSubject: second\r\n\r\nbody";
    let selected = select(
        message,
        &body(BodySection::HeaderFields {
            path: Vec::new(),
            names: vec!["subject".into()],
            excluded: false,
        }),
    )
    .expect("selected fields");
    assert_eq!(
        selected.bytes.as_ref(),
        b"Subject: first\r\nSubject: second\r\n\r\n"
    );
}

/// HEADER.FIELDS.NOT возвращает только поля, чьё имя отсутствует в наборе.
#[test]
fn excludes_named_header_fields() {
    let message = b"Subject: report\r\nX-Trace: id\r\n\r\nbody";
    let selected = select(
        message,
        &body(BodySection::HeaderFields {
            path: Vec::new(),
            names: vec!["SUBJECT".into()],
            excluded: true,
        }),
    )
    .expect("excluded fields");
    assert_eq!(selected.bytes.as_ref(), b"X-Trace: id\r\n\r\n");
}

/// Вложенное message/rfc822 адресуется частью 1 и возвращает его заголовок.
#[test]
fn selects_encapsulated_message_headers() {
    let message = b"Content-Type: message/rfc822\r\n\r\nSubject: nested\r\n\r\nnested body";
    let selected =
        select(message, &body(BodySection::Headers { path: vec![1] })).expect("nested headers");
    assert_eq!(selected.label, "BODY[1.HEADER]");
    assert_eq!(selected.bytes.as_ref(), b"Subject: nested\r\n\r\n");
}

fn body(section: BodySection) -> FetchAttribute {
    FetchAttribute::Body {
        peek: true,
        section,
        partial: None,
    }
}
