//! Проверки wire-сериализации MIME BODYSTRUCTURE.

use super::{encode, encode_extended};

#[test]
fn reports_encoded_octets_and_text_lines() {
    let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\naGVsbG8NCg==";
    let entity = super::super::tree::parse(raw).expect("entity");
    assert_eq!(
        encode(&entity, raw).expect("bodystructure"),
        "(\"TEXT\" \"PLAIN\" (\"CHARSET\" \"utf-8\") NIL NIL \"BASE64\" 12 0)"
    );
}

#[test]
fn encodes_multipart_children_in_order() {
    let raw = b"Content-Type: multipart/alternative; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\na\r\n--x\r\nContent-Type: text/html\r\n\r\n<b>a</b>\r\n--x--\r\n";
    let entity = super::super::tree::parse(raw).expect("entity");
    let encoded = encode(&entity, raw).expect("bodystructure");
    assert!(encoded.ends_with(" \"ALTERNATIVE\")"));
    assert!(encoded.contains("\"TEXT\" \"PLAIN\""));
    assert!(encoded.contains("\"TEXT\" \"HTML\""));
}

#[test]
fn encodes_nested_message_envelope_and_structure() {
    let raw = b"Content-Type: message/rfc822\r\n\r\nFrom: a@test\r\nTo: b@test\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nbody";
    let entity = super::super::tree::parse(raw).expect("entity");
    let encoded = encode(&entity, raw).expect("bodystructure");
    assert!(encoded.starts_with("(\"MESSAGE\" \"RFC822\""));
    assert!(encoded.contains("\"nested\""));
    assert!(encoded.contains("\"TEXT\" \"PLAIN\""));
}

#[test]
fn encodes_headerless_multipart_digest_as_nested_message() {
    let raw = b"Content-Type: multipart/digest; boundary=x\r\n\r\n--x\r\n\r\nSubject: nested\r\n\r\nmessage body\r\n--x--\r\n";
    let entity = super::super::tree::parse(raw).expect("multipart digest");
    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "((\"MESSAGE\" \"RFC822\" NIL NIL NIL \"7BIT\" 31 (NIL \"nested\" NIL NIL NIL NIL NIL NIL NIL NIL) (\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 12 0) 2) \"DIGEST\")"
    );
}

#[test]
fn unknown_transfer_encoding_reports_effective_octet_stream_type() {
    let raw = b"Content-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: x-private\r\n\r\nopaque";
    let entity = super::super::tree::parse(raw).expect("opaque entity");
    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "(\"APPLICATION\" \"OCTET-STREAM\" NIL NIL NIL \"x-private\" 6)"
    );
}

#[test]
fn bodystructure_includes_rfc2183_disposition_after_body_md5_slot() {
    let raw = b"Content-Type: application/pdf\r\nContent-Disposition: attachment; filename*=UTF-8''report%20Q1.pdf\r\n\r\npdf";
    let entity = super::super::tree::parse(raw).expect("MIME entity");
    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "(\"application\" \"pdf\" NIL NIL NIL \"7BIT\" 3 NIL (\"attachment\" (\"FILENAME\" \"report Q1.pdf\")))"
    );
}

#[test]
fn multipart_disposition_follows_subtype_extension_position() {
    let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Disposition: inline\r\n\r\n--x\r\n\r\nbody\r\n--x--\r\n";
    let entity = super::super::tree::parse(raw).expect("multipart entity");
    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "((\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 4 0) \"MIXED\" NIL (\"inline\" NIL))"
    );
}

#[test]
fn message_rfc822_disposition_follows_embedded_message_fields() {
    let raw = b"Content-Type: message/rfc822\r\nContent-Disposition: attachment; filename=forwarded.eml\r\n\r\nSubject: nested\r\n\r\nbody";
    let entity = super::super::tree::parse(raw).expect("message entity");
    assert!(
        encode_extended(&entity, raw)
            .expect("BODYSTRUCTURE")
            .ends_with(" 2 NIL (\"attachment\" (\"FILENAME\" \"forwarded.eml\")))")
    );
}

#[test]
fn language_and_location_extensions_follow_disposition_in_rfc_order() {
    let raw = b"Content-Type: application/pdf\r\nContent-Disposition: attachment\r\nContent-Language: en-US, fr\r\nContent-Location: ../files/report%20one.pdf\r\n\r\npdf";
    let entity = super::super::tree::parse(raw).expect("MIME entity");
    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "(\"application\" \"pdf\" NIL NIL NIL \"7BIT\" 3 NIL (\"attachment\" NIL) (\"en-US\" \"fr\") \"../files/report%20one.pdf\")"
    );
}

#[test]
fn location_only_fills_disposition_and_language_slots_with_nil() {
    let raw = b"Content-Type: application/octet-stream\r\nContent-Location: ./part.bin\r\n\r\nx";
    let entity = super::super::tree::parse(raw).expect("MIME entity");
    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "(\"APPLICATION\" \"OCTET-STREAM\" NIL NIL NIL \"7BIT\" 1 NIL NIL NIL \"./part.bin\")"
    );
}

#[test]
fn multipart_language_and_location_use_parameters_as_first_extension() {
    let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-Language: en\r\nContent-Location: https://example.test/mail/\r\n\r\n--x\r\n\r\nbody\r\n--x--\r\n";
    let entity = super::super::tree::parse(raw).expect("multipart entity");
    assert!(
        encode_extended(&entity, raw)
            .expect("BODYSTRUCTURE")
            .ends_with(" \"MIXED\" NIL NIL \"en\" \"https://example.test/mail/\")")
    );
}

#[test]
fn multipart_content_md5_does_not_occupy_scalar_body_md5_slot() {
    let raw = b"Content-Type: multipart/mixed; boundary=x\r\nContent-MD5: XUFAKrxLKna5cZ2REBfFkg==\r\n\r\n--x\r\n\r\nbody\r\n--x--\r\n";
    let entity = super::super::tree::parse(raw).expect("multipart entity");

    assert_eq!(
        encode_extended(&entity, raw).expect("BODYSTRUCTURE"),
        "((\"TEXT\" \"PLAIN\" NIL NIL NIL \"7BIT\" 4 0) \"MIXED\")"
    );
}
