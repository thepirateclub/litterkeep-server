//! Проверки Korean text, WHATWG labels и строгой обработки malformed input.

use super::super::charset::Charset;
use super::decode;

#[test]
fn decodes_ascii_and_korean_hangul_text() {
    assert_eq!(decode(b"Mail 123"), Ok("Mail 123".to_owned()));
    assert_eq!(
        decode(b"\xbe\xc8\xb3\xe7\xc7\xcf\xbc\xbc\xbf\xe4"),
        Ok("안녕하세요".to_owned())
    );
    assert_eq!(
        Charset::EucKr.decode(b"\xbe\xc8\xb3\xe7"),
        Ok("안녕".to_owned())
    );
}

#[test]
fn whatwg_and_iana_labels_share_typed_variant() {
    for label in [
        "EUC-KR",
        "cseuckr",
        "csEUCKR",
        "csksc56011987",
        "iso-ir-149",
        "korean",
        "ks_c_5601-1987",
        "ks_c_5601-1989",
        "ksc5601",
        "ksc_5601",
        "windows-949",
    ] {
        assert!(
            matches!(Charset::parse(label), Ok(Charset::EucKr)),
            "{label}"
        );
    }
}

#[test]
fn malformed_truncated_and_unmapped_pairs_fail_closed() {
    for bytes in [
        b"\x80".as_slice(),
        b"\xff".as_slice(),
        b"\x81".as_slice(),
        b"\x81\x40".as_slice(),
        b"\xfe\xfe".as_slice(),
    ] {
        assert!(decode(bytes).is_err(), "accepted {bytes:02x?}");
    }
}
