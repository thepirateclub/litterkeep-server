//! Проверки MIME-путей и RFC 9051 BINARY на декодированных октетах.

use super::*;

const MESSAGE: &[u8] = b"MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=parts\r\n\r\n--parts\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\nYmluYXJ5AGF0dGFjaG1lbnQ=\r\n--parts\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nline=0Abreak\r\n--parts--\r\n";

#[test]
fn binary_decodes_base64_and_preserves_nul() {
    let result = select(MESSAGE, &[1], None).expect("valid binary part");
    assert_eq!(result.bytes.as_ref(), b"binary\0attachment");
    assert_eq!(result.label, "BINARY[1]");
}

#[test]
fn binary_partial_applies_after_transfer_decoding() {
    let result = select(
        MESSAGE,
        &[1],
        Some(Partial {
            offset: 7,
            count: 4,
        }),
    )
    .expect("valid partial range");
    assert_eq!(result.bytes.as_ref(), b"atta");
    assert_eq!(result.label, "BINARY[1]<7>");
}

#[test]
fn binary_size_matches_decoded_octet_count() {
    assert_eq!(decoded_size(MESSAGE, &[1]).expect("decoded size"), 17);
}

#[test]
fn text_binary_canonicalizes_line_endings() {
    let result = select(MESSAGE, &[2], None).expect("valid text part");
    assert_eq!(result.bytes.as_ref(), b"line\r\nbreak");
    assert_eq!(decoded_size(MESSAGE, &[2]).expect("decoded text size"), 11);
}

#[test]
fn unsupported_transfer_encoding_keeps_unknown_cte_result() {
    let message = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: x-custom\r\n\r\ndata";
    assert!(matches!(
        select(message, &[1], None),
        Err(FetchError::UnknownContentTransferEncoding)
    ));
}

#[test]
fn binary_rejects_multipart_and_nonexistent_leaf_paths() {
    assert!(matches!(
        select(MESSAGE, &[0], None),
        Err(FetchError::InvalidSection)
    ));
    assert!(matches!(
        select(MESSAGE, &[3], None),
        Err(FetchError::InvalidSection)
    ));
    assert!(matches!(
        select(MESSAGE, &[], None),
        Err(FetchError::InvalidSection)
    ));
}

/// BINARY path проходит через вложенное identity message/global сообщение.
#[test]
fn binary_selects_nested_global_message_attachment() {
    let message = b"Content-Type: message/global\r\n\r\nSubject: nested\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: base64\r\n\r\nbmVzdGVkAA==";
    let result = select(message, &[1, 1], None).expect("nested global attachment");

    assert_eq!(result.label, "BINARY[1.1]");
    assert_eq!(result.bytes.as_ref(), b"nested\0");
}

/// BINARY traversal uses the decoded source beneath Base64 message/global.
#[test]
fn binary_selects_attachment_inside_base64_global_message() {
    let message = b"Content-Type: message/global\r\nContent-Transfer-Encoding: base64\r\n\r\nU3ViamVjdDogbmVzdGVkDQpDb250ZW50LVR5cGU6IGFwcGxpY2F0aW9uL29jdGV0LXN0cmVhbQ0KQ29udGVudC1UcmFuc2Zlci1FbmNvZGluZzogYmFzZTY0DQoNCmJtVnpkR1ZrQUE9PQ==";
    let result = select(message, &[1, 1], None).expect("decoded global attachment");

    assert_eq!(result.label, "BINARY[1.1]");
    assert_eq!(result.bytes.as_ref(), b"nested\0");
}

/// BINARY адресует leaf внутри default MESSAGE/RFC822 части digest-контейнера.
#[test]
fn binary_selects_digest_embedded_message_body_and_size() {
    let message = b"Content-Type: multipart/digest; boundary=x\r\n\r\n--x\r\n\r\nSubject: nested\r\n\r\nmessage body\r\n--x--\r\n";
    let decoded = select(message, &[1, 1], None).expect("digest text body");
    let size = decoded_size(message, &[1, 1]).expect("digest decoded size");

    assert_eq!(decoded.label, "BINARY[1.1]");
    assert_eq!(decoded.bytes.as_ref(), b"message body");
    assert_eq!(size, decoded.bytes.len());
}

/// Проверяет одинаковую embedded нумерацию во всех основных multipart-профилях.
#[test]
fn binary_traverses_embedded_message_in_each_multipart_profile() {
    let fixtures = [
        (
            MediaType::MultipartMixed,
            b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nmessage body\r\n--x--\r\n".as_slice(),
        ),
        (
            MediaType::MultipartAlternative,
            b"Content-Type: multipart/alternative; boundary=x\r\n\r\n--x\r\nContent-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nmessage body\r\n--x\r\nContent-Type: text/plain\r\n\r\nalternative\r\n--x--\r\n".as_slice(),
        ),
        (
            MediaType::MultipartRelated,
            b"Content-Type: multipart/related; boundary=x; type=\"message/rfc822\"\r\n\r\n--x\r\nContent-Type: message/rfc822\r\n\r\nSubject: nested\r\nContent-Type: text/plain\r\n\r\nmessage body\r\n--x--\r\n".as_slice(),
        ),
        (
            MediaType::MultipartDigest,
            b"Content-Type: multipart/digest; boundary=x\r\n\r\n--x\r\n\r\nSubject: nested\r\n\r\nmessage body\r\n--x--\r\n".as_slice(),
        ),
        (
            MediaType::MultipartMixed,
            b"Content-Type: multipart/mixed; boundary=x\r\n\r\n--x\r\nContent-Type: message/global\r\n\r\nSubject: nested\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nmessage body\r\n--x--\r\n".as_slice(),
        ),
    ];

    for (expected_type, message) in fixtures {
        let tree = tree::parse(message).expect("multipart MIME tree");
        assert_eq!(tree.media_type, expected_type);
        let decoded = select(message, &[1, 1], None).expect("embedded text leaf");
        let size = decoded_size(message, &[1, 1]).expect("embedded text size");

        assert_eq!(decoded.bytes.as_ref(), b"message body");
        assert_eq!(size, decoded.bytes.len());
    }
}
