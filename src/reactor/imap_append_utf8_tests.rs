//! Проверки границы между UTF-8 заголовками и произвольным телом APPEND.

use super::has_eight_bit_header;

#[test]
fn detects_eight_bit_octets_inside_header_fields() {
    assert!(has_eight_bit_header("Subject: café\r\n\r\nbody".as_bytes()));
}

#[test]
fn accepts_eight_bit_octets_after_header_separator() {
    assert!(!has_eight_bit_header(b"Subject: invoice\r\n\r\n\xff\x80"));
}

#[test]
fn empty_header_block_does_not_scan_message_body() {
    assert!(!has_eight_bit_header(b"\r\n\xff"));
}

#[test]
fn conservatively_detects_eight_bit_octets_without_separator() {
    assert!(has_eight_bit_header(b"Subject: caf\xff"));
}
