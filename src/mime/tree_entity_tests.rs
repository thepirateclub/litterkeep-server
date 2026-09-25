//! Регрессии разбора MIME entity и сохранения абсолютных диапазонов.

use super::{MediaType, parse};

#[test]
fn boundary_substring_does_not_split_and_ranges_preserve_parts() {
    let raw = b"Content-Type: multipart/mixed; boundary=x\r\n\r\npreamble\r\n--x\r\nContent-Type: text/plain\r\n\r\ninside --x text\r\n--x\r\nContent-Type: text/html\r\n\r\n<b>x</b>\r\n--x--\r\nepilogue";
    let tree = parse(raw).expect("multipart");
    assert_eq!(tree.children.len(), 2);
    let first = tree.children.first().expect("first part");
    assert_eq!(
        raw.get(first.body_range.clone()),
        Some(b"inside --x text".as_slice())
    );
    let epilogue = tree.epilogue_range.expect("epilogue");
    assert_eq!(raw.get(epilogue), Some(b"epilogue".as_slice()));
}

#[test]
fn missing_top_level_closing_delimiter_is_rejected() {
    let raw = b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nbody";
    assert!(parse(raw).is_err());
}

#[test]
fn outer_boundary_terminates_truncated_nested_multipart() {
    let raw = b"Content-Type: multipart/mixed; boundary=outer\r\n\r\n--outer\r\nContent-Type: multipart/mixed; boundary=middle\r\n\r\n--middle\r\nContent-Type: multipart/mixed; boundary=inner\r\n\r\n--inner\r\nContent-Type: text/plain\r\n\r\nnested body\r\n--outer\r\nContent-Type: text/plain\r\n\r\nsibling\r\n--outer--\r\n";
    let tree = parse(raw).expect("outer multipart closes nested entity");

    assert_eq!(tree.children.len(), 2);
    let middle = tree.children.first().expect("middle multipart part");
    let nested = middle.children.first().expect("inner multipart part");
    let nested_body = nested.children.first().expect("nested text part");
    assert_eq!(middle.children.len(), 1);
    assert_eq!(nested.children.len(), 1);
    assert_eq!(
        raw.get(nested_body.body_range.clone()),
        Some(b"nested body".as_slice())
    );
    let sibling = tree.children.get(1).expect("outer sibling part");
    assert_eq!(
        raw.get(sibling.body_range.clone()),
        Some(b"sibling".as_slice())
    );
}

#[test]
fn outer_boundary_policy_crosses_message_rfc822_nesting() {
    let raw = b"Content-Type: multipart/mixed; boundary=outer\r\n\r\n--outer\r\nContent-Type: message/rfc822\r\n\r\nFrom: alice@example.test\r\nContent-Type: multipart/mixed; boundary=inner\r\n\r\n--inner\r\nContent-Type: text/plain\r\n\r\nembedded body\r\n--outer\r\nContent-Type: text/plain\r\n\r\nsibling\r\n--outer--\r\n";
    let tree = parse(raw).expect("outer delimiter crosses encapsulated message");
    let message = tree.children.first().expect("message/rfc822 part");
    let nested_multipart = message.children.first().expect("embedded multipart");
    let embedded = nested_multipart
        .children
        .first()
        .expect("embedded text part");

    assert_eq!(
        raw.get(embedded.body_range.clone()),
        Some(b"embedded body".as_slice())
    );
    assert_eq!(tree.children.len(), 2);
}

#[test]
fn leaf_decodes_only_its_raw_body_range() {
    let raw = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8A";
    let entity = parse(raw).expect("entity");
    assert_eq!(entity.decoded_body(raw).expect("decoded body"), b"hello\0");
}

#[test]
fn mime_base64_ignores_non_alphabet_octets() {
    let raw = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\n!aGVsbG8=";
    let entity = parse(raw).expect("MIME entity");
    assert_eq!(entity.decoded_body(raw).expect("decoded body"), b"hello");
}

#[test]
fn message_rfc822_contains_nested_entity_with_absolute_ranges() {
    let raw = b"Content-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nbody";
    let entity = parse(raw).expect("message entity");
    let nested = entity.children.first().expect("nested entity");
    assert_eq!(raw.get(nested.body_range.clone()), Some(b"body".as_slice()));
}

#[test]
fn rejects_transforming_and_unknown_cte_on_composite_entities() {
    for content_type in ["multipart/mixed; boundary=x", "message/partial"] {
        for transfer_encoding in ["base64", "quoted-printable", "x-private"] {
            let raw = format!(
                "Content-Type: {content_type}\r\nContent-Transfer-Encoding: {transfer_encoding}\r\n\r\nbody"
            );
            assert!(
                parse(raw.as_bytes()).is_err(),
                "accepted {content_type} with {transfer_encoding}"
            );
        }
    }

    let duplicate = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Transfer-Encoding: base64\r\nContent-Transfer-Encoding: 8bit\r\n\r\nbody";
    assert!(parse(duplicate).is_err());
}

#[test]
fn permits_identity_cte_on_multipart_entity() {
    let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Transfer-Encoding: 8bit\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nbody\r\n--x--\r\n";
    assert!(parse(raw).is_ok());
}

#[test]
fn unknown_leaf_encoding_is_opaque_without_rewriting_source_headers() {
    let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: x-private\r\n\r\nbody";
    let entity = parse(raw).expect("opaque MIME leaf");

    assert_eq!(entity.media_type, MediaType::ApplicationOctetStream);
    assert!(entity.parameters.is_empty());
    assert_eq!(
        entity.headers.first().map(|field| field.value.as_slice()),
        Some(b"text/plain; charset=utf-8".as_slice())
    );
    assert_eq!(raw.get(entity.body_range), Some(b"body".as_slice()));
}

#[test]
fn duplicate_cte_fields_are_rejected_instead_of_selecting_the_last() {
    let raw = b"Content-Type: text/plain\r\nContent-Transfer-Encoding: x-private\r\nContent-Transfer-Encoding: 8bit\r\n\r\nbody";
    assert!(parse(raw).is_err());
}

#[test]
fn malformed_content_type_falls_back_to_plain_text_and_keeps_header() {
    let raw = b"Content-Type: text/plain; charset=\"unterminated\r\n\r\nplain body";
    let entity = parse(raw).expect("RFC 2045 default type");

    assert_eq!(entity.media_type, MediaType::TextPlain);
    assert!(entity.parameters.is_empty());
    assert_eq!(raw.get(entity.body_range), Some(b"plain body".as_slice()));
    assert_eq!(
        entity.headers.first().map(|field| field.value.as_slice()),
        Some(b"text/plain; charset=\"unterminated".as_slice())
    );
}

#[test]
fn duplicate_content_type_fields_are_rejected_as_ambiguous() {
    let raw = b"Content-Type: text/plain\r\nContent-Type: application/octet-stream\r\n\r\nbody";
    assert!(parse(raw).is_err());
}

#[test]
fn multipart_rejects_a_boundary_longer_than_seventy_octets() {
    let boundary = "a".repeat(71);
    let raw = format!(
        "Content-Type: multipart/mixed; boundary={boundary}\r\n\r\n--{boundary}\r\n\r\nbody\r\n--{boundary}--\r\n"
    );
    assert!(parse(raw.as_bytes()).is_err());
}

#[test]
fn multipart_stops_at_the_remaining_global_part_budget() {
    let mut raw = String::from("Content-Type: multipart/mixed; boundary=x\r\n\r\n");
    for _ in 0..256 {
        raw.push_str("--x\r\n\r\npart\r\n");
    }
    raw.push_str("--x--\r\n");
    assert!(parse(raw.as_bytes()).is_err());
}
