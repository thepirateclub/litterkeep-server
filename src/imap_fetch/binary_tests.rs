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
