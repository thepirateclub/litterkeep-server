//! Проверки EUC-JP byte forms, aliases и строгого отказа на ошибки.

use super::super::charset::Charset;
use super::decode;

#[test]
fn decodes_ascii_japanese_and_halfwidth_kana() {
    assert_eq!(decode(b"ASCII"), Ok("ASCII".to_owned()));
    assert_eq!(decode(b"\xc6\xfc\xcb\xdc\xb8\xec"), Ok("日本語".to_owned()));
    assert_eq!(decode(b"\x8e\xb6\x8e\xc0"), Ok("ｶﾀ".to_owned()));
    assert_eq!(decode(b"\x8f\xa2\xaf\x8f\xa2\xb0"), Ok("˘ˇ".to_owned()));
}

#[test]
fn canonical_name_and_registered_aliases_share_the_typed_variant() {
    for label in ["EUC-JP", "x-euc-jp", "csEUCPkdFmtJapanese"] {
        assert!(
            matches!(Charset::parse(label), Ok(Charset::EucJp)),
            "{label}"
        );
    }
    assert_eq!(Charset::EucJp.decode(b"\xc6\xfc"), Ok("日".to_owned()));
}

#[test]
fn malformed_and_unmapped_sequences_fail_closed() {
    for bytes in [
        b"\x80".as_slice(),
        b"\xa1".as_slice(),
        b"\xa1\x20".as_slice(),
        b"\x8e".as_slice(),
        b"\x8e\xa0".as_slice(),
        b"\x8f\xa1".as_slice(),
        b"\x8f\xa1\x20".as_slice(),
        b"\x8f\xa1\xa1".as_slice(),
    ] {
        assert!(decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}
