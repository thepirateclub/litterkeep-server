//! Unit matrix GBK/GB18030 vectors, aliases и malformed sequences.

use super::super::charset::Charset;
use super::decode;

#[test]
fn decodes_ascii_euro_chinese_and_supplementary_character() {
    assert_eq!(decode(b"ASCII \x80"), Ok("ASCII €".to_owned()));
    assert_eq!(decode(b"\xd6\xd0\xce\xc4"), Ok("中文".to_owned()));
    assert_eq!(decode(b"\x949\xfc6"), Ok("😀".to_owned()));
}

#[test]
fn canonical_and_registered_aliases_use_the_same_decoder() {
    for label in [
        "GBK",
        "csGBK",
        "CP936",
        "MS936",
        "windows-936",
        "gb2312",
        "chinese",
        "x-gbk",
    ] {
        assert!(matches!(Charset::parse(label), Ok(Charset::Gbk)), "{label}");
    }
    for label in ["GB18030", "csGB18030"] {
        assert!(
            matches!(Charset::parse(label), Ok(Charset::Gb18030)),
            "{label}"
        );
    }
    assert_eq!(Charset::Gbk.decode(b"\xd6\xd0"), Ok("中".to_owned()));
    assert_eq!(Charset::Gb18030.decode(b"\x949\xfc6"), Ok("😀".to_owned()));
}

#[test]
fn rejects_truncated_invalid_and_unmapped_sequences() {
    for bytes in [
        b"\x81".as_slice(),
        b"\xff".as_slice(),
        b"\x81\x7f".as_slice(),
        b"\x81\x30".as_slice(),
        b"\x81\x30\x81".as_slice(),
        b"\x81\x30\x81\x20".as_slice(),
        b"\x810000".as_slice(),
    ] {
        assert!(decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}
