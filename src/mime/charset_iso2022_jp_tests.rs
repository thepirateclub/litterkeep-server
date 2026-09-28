//! Проверки ISO-2022-JP mode transitions и malformed escape rejection.

use super::super::charset::Charset;
use super::decode;

#[test]
fn decodes_jis_ascii_roman_and_katakana_modes() {
    assert_eq!(
        decode(b"\x1b$B\x46\x7c\x4b\x5c\x38\x6c\x1b(B"),
        Ok("日本語".to_owned())
    );
    assert_eq!(decode(b"\x1b(J\\~\x1b(B"), Ok("¥‾".to_owned()));
    assert_eq!(decode(b"\x1b(I\x36\x40\x1b(B"), Ok("ｶﾀ".to_owned()));
    assert_eq!(decode(b"A\x1b$B\x46\x7c\x1b(BZ"), Ok("A日Z".to_owned()));
}

#[test]
fn registered_charset_labels_use_the_typed_variant() {
    for label in ["ISO-2022-JP", "csISO2022JP"] {
        assert!(
            matches!(Charset::parse(label), Ok(Charset::Iso2022Jp)),
            "{label}"
        );
    }
    assert_eq!(
        Charset::Iso2022Jp.decode(b"\x1b$B\x46\x7c\x1b(B"),
        Ok("日".to_owned())
    );
}

#[test]
fn malformed_transitions_and_truncated_sequences_fail_closed() {
    for bytes in [
        b"\x0e".as_slice(),
        b"\x1b".as_slice(),
        b"\x1bX".as_slice(),
        b"\x1b$".as_slice(),
        b"\x1b$B\x46".as_slice(),
        b"\x1b$B\x46\x7c".as_slice(),
        b"\x1b(J\\".as_slice(),
        b"\x1b(I\x36".as_slice(),
        b"\x1b$B\x20\x21".as_slice(),
        b"\x1b(B\x1b(J".as_slice(),
    ] {
        assert!(decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}
