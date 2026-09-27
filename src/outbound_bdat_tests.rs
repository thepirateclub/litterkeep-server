//! Unit проверки bounded RFC 3030 chunk boundaries и MIME validation.
use super::{MAX_CHUNK_SIZE, binary_chunks, has_binary_transfer, validate_binary};

/// Размер chunks ограничен, а LAST выставляется ровно на последнем участке.
#[test]
fn chunks_binary_message_without_copying_or_losing_last_marker() {
    let message = vec![0xA5; MAX_CHUNK_SIZE.saturating_add(3)];
    let chunks = binary_chunks(&message).collect::<Vec<_>>();
    let mut chunks = chunks.iter();
    let first = chunks.next();
    let second = chunks.next();

    assert_eq!(first.map(|chunk| chunk.bytes.len()), Some(MAX_CHUNK_SIZE));
    assert_eq!(first.map(|chunk| chunk.last), Some(false));
    assert_eq!(second.map(|chunk| chunk.bytes.len()), Some(3));
    assert_eq!(second.map(|chunk| chunk.last), Some(true));
}

/// Пустое сообщение представляется обязательным нулевым LAST chunk.
#[test]
fn empty_message_has_one_zero_length_last_chunk() {
    let chunks = binary_chunks(&[]).collect::<Vec<_>>();
    let chunk = chunks.first();

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunk.map(|chunk| chunk.bytes.len()), Some(0));
    assert_eq!(chunk.map(|chunk| chunk.last), Some(true));
}

/// Нулевой и high-bit octets остаются непрозрачными внутри application leaf.
#[test]
fn binary_mime_validation_preserves_arbitrary_application_octets() {
    let message = b"Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xff\r\n\n\r";

    assert!(has_binary_transfer(message));
    assert!(validate_binary(message));
}

/// Text MIME часть обязана сохранить canonical CRLF, даже в BINARYMIME письме.
#[test]
fn binary_mime_validation_rejects_noncanonical_text_part_lines() {
    let message = b"Content-Type: text/plain\r\nContent-Transfer-Encoding: binary\r\n\r\ntext\n";

    assert!(!validate_binary(message));
}

/// RFC 3030 canonical CRLF не наследует DATA-командный предел в 1000 октетов.
#[test]
fn binary_mime_allows_canonical_text_lines_above_data_line_limit() {
    let mut message =
        b"Content-Type: text/plain\r\nContent-Transfer-Encoding: binary\r\n\r\n".to_vec();
    message.extend(std::iter::repeat_n(b'x', 1_200));
    message.extend_from_slice(b"\r\n");

    assert!(validate_binary(&message));
}

/// Даже при BINARYMIME текст multipart preamble и epilogue используют CRLF.
#[test]
fn binary_mime_checks_multipart_text_outside_leaf_parts() {
    let prefix = b"Content-Type: multipart/mixed; boundary=part\r\n\r\n";
    let suffix = b"--part\r\nContent-Type: application/octet-stream\r\nContent-Transfer-Encoding: binary\r\n\r\n\0\xff\r\n--part--\r\nepilogue\r\n";
    let invalid = [prefix.as_slice(), b"preamble\n", suffix.as_slice()].concat();
    let valid = [prefix.as_slice(), b"preamble\r\n", suffix.as_slice()].concat();

    assert!(!validate_binary(&invalid));
    assert!(validate_binary(&valid));
}
