//! Регрессии SMTP DATA transparency, лимитов и SMTPUTF8 octet policy.
use super::{append_data_line, discard_overlong_incomplete_line};
use crate::reactor::session::DataSection;
use crate::smtp::BodyMode;

/// Передаёт ровно одну wire-строку вместе с устойчивым DATA parser state.
fn append_line(
    bytes: &mut Vec<u8>,
    section: &mut DataSection,
    oversized: &mut bool,
    mode: BodyMode,
    smtp_utf8: bool,
    wire: &[u8],
) {
    append_data_line(bytes, section, oversized, mode, smtp_utf8, wire).expect("DATA line");
}

/// Длинный фрагмент DATA освобождает входной буфер до получения CRLF.
#[test]
fn overlong_incomplete_data_line_is_discarded_without_losing_terminator_sync() {
    let mut incomplete = vec![b'x'; 1001];
    let mut oversized = false;
    discard_overlong_incomplete_line(&mut incomplete, &mut oversized);
    assert!(oversized);
    assert!(incomplete.is_empty());

    let mut bounded_fragment = vec![b'x'; 1000];
    let mut still_valid = false;
    discard_overlong_incomplete_line(&mut bounded_fragment, &mut still_valid);
    assert!(!still_valid);
    assert_eq!(bounded_fragment.len(), 1000);

    let mut complete_line = vec![b'x'; 1001];
    complete_line.extend_from_slice(b"\r\n");
    let mut complete_oversized = false;
    discard_overlong_incomplete_line(&mut complete_line, &mut complete_oversized);
    assert!(!complete_oversized);
    assert_eq!(complete_line.len(), 1003);

    let mut trailing_fragment = b"valid\r\n".to_vec();
    trailing_fragment.extend(vec![b'x'; 1001]);
    let mut trailing_oversized = false;
    discard_overlong_incomplete_line(&mut trailing_fragment, &mut trailing_oversized);
    assert!(trailing_oversized);
    assert_eq!(trailing_fragment, b"valid\r\n");
}

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
        let mut section = DataSection::Headers;
        let mut oversized = false;
        append_line(
            &mut bytes,
            &mut section,
            &mut oversized,
            BodyMode::SevenBit,
            false,
            wire,
        );
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
    let mut section = DataSection::Headers;
    let mut oversized = false;
    append_line(
        &mut bytes,
        &mut section,
        &mut oversized,
        BodyMode::SevenBit,
        false,
        accepted.as_bytes(),
    );
    assert!(!oversized);
    assert_eq!(bytes.len(), 1000);

    append_line(
        &mut bytes,
        &mut section,
        &mut oversized,
        BodyMode::SevenBit,
        false,
        rejected.as_bytes(),
    );
    assert!(oversized);
    assert_eq!(bytes.len(), 1000);
}

/// UTF-8 заголовки и 8-bit MIME body требуют независимые MAIL extensions.
#[test]
fn header_and_body_octets_require_their_own_mail_extensions() {
    let header = "Subject: Привет\r\n".as_bytes();
    let mut accepted = Vec::new();
    let mut accepted_section = DataSection::Headers;
    let mut rejected = false;
    append_line(
        &mut accepted,
        &mut accepted_section,
        &mut rejected,
        BodyMode::SevenBit,
        true,
        header,
    );
    assert!(!rejected);

    let mut rejected_header = Vec::new();
    let mut rejected_header_section = DataSection::Headers;
    let mut rejected = false;
    append_line(
        &mut rejected_header,
        &mut rejected_header_section,
        &mut rejected,
        BodyMode::EightBitMime,
        false,
        header,
    );
    assert!(rejected);

    let mut non_ascii_name = Vec::new();
    let mut non_ascii_name_section = DataSection::Headers;
    let mut rejected = false;
    append_line(
        &mut non_ascii_name,
        &mut non_ascii_name_section,
        &mut rejected,
        BodyMode::EightBitMime,
        true,
        "Заголовок: value\r\n".as_bytes(),
    );
    assert!(rejected);

    let mut malformed_utf8 = Vec::new();
    let mut malformed_utf8_section = DataSection::Headers;
    let mut rejected = false;
    append_line(
        &mut malformed_utf8,
        &mut malformed_utf8_section,
        &mut rejected,
        BodyMode::EightBitMime,
        true,
        b"Subject: \xff\r\n",
    );
    assert!(rejected);

    let mut body = Vec::new();
    let mut body_section = DataSection::Headers;
    let mut rejected_body = false;
    append_line(
        &mut body,
        &mut body_section,
        &mut rejected_body,
        BodyMode::SevenBit,
        true,
        header,
    );
    append_line(
        &mut body,
        &mut body_section,
        &mut rejected_body,
        BodyMode::SevenBit,
        true,
        b"\r\n",
    );
    assert!(matches!(body_section, DataSection::Body));
    append_line(
        &mut body,
        &mut body_section,
        &mut rejected_body,
        BodyMode::SevenBit,
        true,
        "Привет\r\n".as_bytes(),
    );
    assert!(rejected_body);

    let mut headerless_body = Vec::new();
    let mut headerless_section = DataSection::Headers;
    let mut rejected_headerless = false;
    append_line(
        &mut headerless_body,
        &mut headerless_section,
        &mut rejected_headerless,
        BodyMode::EightBitMime,
        false,
        b"\r\n",
    );
    append_line(
        &mut headerless_body,
        &mut headerless_section,
        &mut rejected_headerless,
        BodyMode::EightBitMime,
        false,
        "Привет\r\n".as_bytes(),
    );
    assert!(!rejected_headerless);
}
