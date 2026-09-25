//! Регрессии SMTP DATA transparency, лимитов и SMTPUTF8 octet policy.
use super::append_data_line;
use crate::smtp::BodyMode;

/// Каждая DATA wire-строка снимает ровно один transparency dot.
#[test]
fn data_removes_exactly_one_transparency_dot_from_every_dotted_line() {
    let cases: [(&[u8], &[u8]); 3] = [
        (b"..one\r\n", b".one\r\n"),
        (b"...two\r\n", b"..two\r\n"),
        (b"....three\r\n", b"...three\r\n"),
    ];
    for (wire, expected) in cases {
        let mut bytes = Vec::new();
        let mut oversized = false;
        append_data_line(&mut bytes, &mut oversized, BodyMode::SevenBit, false, wire)
            .expect("DATA line");
        assert!(!oversized);
        assert_eq!(bytes, expected);
    }
}

/// Граница 1000 octets считается после unstuffing и включает CRLF.
#[test]
fn data_line_limit_counts_crlf_but_excludes_the_transparency_dot() {
    let accepted = format!("..{}\r\n", "x".repeat(997));
    let rejected = format!("{}\r\n", "x".repeat(999));
    let mut bytes = Vec::new();
    let mut oversized = false;
    append_data_line(
        &mut bytes,
        &mut oversized,
        BodyMode::SevenBit,
        false,
        accepted.as_bytes(),
    )
    .expect("boundary DATA line");
    assert!(!oversized);
    assert_eq!(bytes.len(), 1000);

    append_data_line(
        &mut bytes,
        &mut oversized,
        BodyMode::SevenBit,
        false,
        rejected.as_bytes(),
    )
    .expect("oversized DATA line");
    assert!(oversized);
    assert_eq!(bytes.len(), 1000);
}

/// UTF-8 заголовки и 8-bit MIME body требуют независимые MAIL extensions.
#[test]
fn header_and_body_octets_require_their_own_mail_extensions() {
    let header = "Subject: Привет\r\n".as_bytes();
    let mut accepted = Vec::new();
    let mut rejected = false;
    append_data_line(
        &mut accepted,
        &mut rejected,
        BodyMode::SevenBit,
        true,
        header,
    )
    .expect("SMTPUTF8 header");
    assert!(!rejected);

    let mut rejected_header = Vec::new();
    let mut rejected = false;
    append_data_line(
        &mut rejected_header,
        &mut rejected,
        BodyMode::EightBitMime,
        false,
        header,
    )
    .expect("header without SMTPUTF8");
    assert!(rejected);

    let mut non_ascii_name = Vec::new();
    let mut rejected = false;
    append_data_line(
        &mut non_ascii_name,
        &mut rejected,
        BodyMode::EightBitMime,
        true,
        "Заголовок: value\r\n".as_bytes(),
    )
    .expect("non-ASCII field name");
    assert!(rejected);

    let mut malformed_utf8 = Vec::new();
    let mut rejected = false;
    append_data_line(
        &mut malformed_utf8,
        &mut rejected,
        BodyMode::EightBitMime,
        true,
        b"Subject: \xff\r\n",
    )
    .expect("malformed UTF-8 field value");
    assert!(rejected);

    let mut body = b"Subject: x\r\n\r\n".to_vec();
    let mut rejected_body = false;
    append_data_line(
        &mut body,
        &mut rejected_body,
        BodyMode::EightBitMime,
        false,
        "Привет\r\n".as_bytes(),
    )
    .expect("8BITMIME body");
    assert!(!rejected_body);

    let mut headerless_body = Vec::new();
    let mut rejected_headerless = false;
    append_data_line(
        &mut headerless_body,
        &mut rejected_headerless,
        BodyMode::EightBitMime,
        false,
        b"\r\n",
    )
    .expect("empty header separator");
    append_data_line(
        &mut headerless_body,
        &mut rejected_headerless,
        BodyMode::EightBitMime,
        false,
        "Привет\r\n".as_bytes(),
    )
    .expect("8BITMIME body after empty headers");
    assert!(!rejected_headerless);
}
