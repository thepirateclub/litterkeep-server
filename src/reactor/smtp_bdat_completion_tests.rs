//! Проверяет выбранный SMTP body profile до передачи сообщения в Store.
use super::{Transaction, valid_message};
use crate::smtp::BodyMode;

/// Создаёт transaction непосредственно на границе валидатора BDAT LAST.
fn transaction(bytes: &[u8], body_mode: BodyMode, smtp_utf8: bool) -> Transaction {
    Transaction {
        bytes: bytes.to_vec(),
        reverse_path: "alice@test".to_owned(),
        recipients: vec!["bob@test".to_owned()],
        body_mode,
        smtp_utf8,
        failed: false,
    }
}

/// BINARYMIME пропускает arbitrary octets для binary MIME leaf без потерь.
#[test]
fn binarymime_accepts_opaque_application_octets() {
    let message = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xffbare\ncr\rbytes";

    assert!(valid_message(&transaction(
        message,
        BodyMode::BinaryMime,
        false
    )));
}

/// RFC 3030 сохраняет обязательный canonical CRLF в каждой text/* части.
#[test]
fn binarymime_rejects_noncanonical_text_leaf() {
    let message = b"Content-Type: text/plain\r\nContent-Transfer-Encoding: binary\r\n\r\nline one\nline two\r\n";

    assert!(!valid_message(&transaction(
        message,
        BodyMode::BinaryMime,
        false
    )));
}

/// DATA line cap не отбрасывает длинную canonical text line в режиме BDAT.
#[test]
fn binarymime_accepts_canonical_text_line_over_data_limit() {
    let mut message =
        b"Content-Type: text/plain\r\nContent-Transfer-Encoding: binary\r\n\r\n".to_vec();
    message.extend(std::iter::repeat_n(b'x', 1_200));
    message.extend_from_slice(b"\r\n");

    assert!(valid_message(&transaction(
        &message,
        BodyMode::BinaryMime,
        false
    )));
}

/// RFC 6532 UTF-8 header поля требует MAIL параметр SMTPUTF8 и в BINARYMIME.
#[test]
fn binarymime_utf8_headers_require_smtp_utf8_mail_flag() {
    let mut message = "Subject: Привет\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n"
        .as_bytes()
        .to_vec();
    message.extend_from_slice(b"\0\xff");

    assert!(!valid_message(&transaction(
        &message,
        BodyMode::BinaryMime,
        false
    )));
    assert!(valid_message(&transaction(
        &message,
        BodyMode::BinaryMime,
        true
    )));
}
