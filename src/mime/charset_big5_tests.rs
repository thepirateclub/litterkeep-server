//! Тесты стандартных Big5 mappings, aliases и отказов malformed sequences.

use super::super::charset::Charset;
use super::decode;

#[test]
fn decodes_ascii_and_traditional_chinese() {
    assert_eq!(decode(b"Big5 \xa4\xa4\xa4\xe5"), Ok("Big5 中文".to_owned()));
    assert_eq!(decode(b"\xa7\x41\xa6\x6e"), Ok("你好".to_owned()));
}

#[test]
fn decodes_all_four_two_scalar_expansion_exceptions() {
    assert_eq!(decode(b"\x88\x62"), Ok("Ê\u{0304}".to_owned()));
    assert_eq!(decode(b"\x88\x64"), Ok("Ê\u{030c}".to_owned()));
    assert_eq!(decode(b"\x88\xa3"), Ok("ê\u{0304}".to_owned()));
    assert_eq!(decode(b"\x88\xa5"), Ok("ê\u{030c}".to_owned()));
}

#[test]
fn canonical_name_and_whatwg_labels_map_to_big5_enum() {
    for label in ["Big5", "csBig5", "big5-hkscs", "cn-big5", "x-x-big5"] {
        assert!(
            matches!(Charset::parse(label), Ok(Charset::Big5)),
            "{label}"
        );
    }
    assert_eq!(Charset::Big5.decode(b"\xa4\xa4"), Ok("中".to_owned()));
}

#[test]
fn truncated_invalid_and_unmapped_sequences_fail_closed() {
    for bytes in [
        b"\x81".as_slice(),
        b"\x80".as_slice(),
        b"\x81\x7f".as_slice(),
        b"\x81\xa0".as_slice(),
    ] {
        assert!(decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}
