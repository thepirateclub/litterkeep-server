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

/// Номер один простого сообщения выбирает его единственный body part.
#[test]
fn selects_top_level_single_part_body_as_part_one() {
    let message = b"Content-Type: text/plain\r\n\r\nsimple body";
    let selected = select(
        message,
        &body(BodySection::Part {
            path: vec![1],
            mime_headers: false,
        }),
    )
    .expect("top-level part one");

    assert_eq!(selected.label, "BODY[1]");
    assert_eq!(selected.bytes.as_ref(), b"simple body");
}

/// Нумерация multipart учитывает вложенные части encoded message/global.
#[test]
fn selects_multipart_global_part_and_its_decoded_body_part() {
    let message = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: message/global\r\nContent-Transfer-Encoding: base64\r\n\r\nU3ViamVjdDogbmVzdGVkDQpDb250ZW50LVR5cGU6IHRleHQvcGxhaW4NCg0KYm9keQ==\r\n--x--\r\n";
    let encoded = select(
        message,
        &body(BodySection::Part {
            path: vec![1],
            mime_headers: false,
        }),
    )
    .expect("encoded message/global MIME body");
    let nested = select(
        message,
        &body(BodySection::Part {
            path: vec![1, 1],
            mime_headers: false,
        }),
    )
    .expect("nested message/global body part");

    assert_eq!(encoded.label, "BODY[1]");
    assert_eq!(
        encoded.bytes.as_ref(),
        b"U3ViamVjdDogbmVzdGVkDQpDb250ZW50LVR5cGU6IHRleHQvcGxhaW4NCg0KYm9keQ=="
    );
    assert_eq!(nested.label, "BODY[1.1]");
    assert_eq!(nested.bytes.as_ref(), b"body");
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

/// HEADER.FIELDS partial применяется после отбора, а не к исходному header.
#[test]
fn header_fields_partial_slices_the_filtered_subset() {
    let inclusion = b"X-Ignored: before selected field\r\nSubject: report\r\n\r\nbody";
    let exclusion =
        b"Subject: report\r\nFrom: bob@example.test\r\nX-Ignored: later field\r\n\r\nbody";
    let cases = [
        (
            inclusion.as_slice(),
            BodySection::HeaderFields {
                path: Vec::new(),
                names: vec!["subject".into()],
                excluded: false,
            },
            b"Subject:".as_slice(),
        ),
        (
            exclusion.as_slice(),
            BodySection::HeaderFields {
                path: Vec::new(),
                names: vec!["SUBJECT".into()],
                excluded: true,
            },
            b"From: bo".as_slice(),
        ),
    ];

    for (message, section, expected) in cases {
        let attribute = FetchAttribute::Body {
            peek: true,
            section,
            partial: Some(Partial {
                offset: 0,
                count: expected.len(),
            }),
        };
        let result = select(message, &attribute).expect("partial header subset");
        assert_eq!(result.bytes.as_ref(), expected);
        assert!(result.label.ends_with("<0>"));
    }
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
