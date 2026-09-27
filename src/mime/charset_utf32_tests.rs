//! Проверки IANA UTF-32 byte order, Unicode scalars и MIME labels.

use super::{ByteOrder, decode};
use crate::mime::charset::Charset;

#[test]
/// Проверяет canonical labels без выдумывания незарегистрированных aliases.
fn parses_only_the_three_iana_utf32_names() {
    assert_eq!(Charset::parse("UTF-32"), Ok(Charset::Utf32));
    assert_eq!(Charset::parse("utf-32be"), Ok(Charset::Utf32Be));
    assert_eq!(Charset::parse("Utf-32Le"), Ok(Charset::Utf32Le));
    assert!(Charset::parse("csUTF32").is_err());
}

#[test]
/// Generic UTF-32 strips only its registered BE BOM and defaults to BE.
fn generic_utf32_uses_big_endian_and_accepts_its_bom() {
    assert_eq!(
        decode(&[0, 0, 0, b'A'], ByteOrder::DetectBom),
        Ok("A".into())
    );
    assert_eq!(
        decode(&[0, 0, 0xfe, 0xff, 0, 0, 0, b'A'], ByteOrder::DetectBom),
        Ok("A".into())
    );
}

#[test]
/// Explicit endian labels decode supplementary scalars without BOM guessing.
fn explicit_endian_profiles_decode_supplementary_scalars() {
    assert_eq!(
        decode(&[0, 1, 0xf6, 0], ByteOrder::BigEndian),
        Ok("😀".into())
    );
    assert_eq!(
        decode(&[0, 0xf6, 1, 0], ByteOrder::LittleEndian),
        Ok("😀".into())
    );
}

#[test]
/// Неполные units, surrogate values и значения выше Unicode отклоняются.
fn rejects_malformed_units_and_non_scalar_values() {
    assert!(decode(&[0, 0, 0], ByteOrder::BigEndian).is_err());
    assert!(decode(&[0, 0, 0xd8, 0], ByteOrder::BigEndian).is_err());
    assert!(decode(&[0, 0x11, 0, 0], ByteOrder::BigEndian).is_err());
}

#[test]
/// UTF-32 не допускается в MIME text/* projection по IANA registration.
fn utf32_profiles_are_not_mime_text_charsets() {
    for charset in [Charset::Utf32, Charset::Utf32Be, Charset::Utf32Le] {
        assert!(!charset.is_suitable_for_text_mime());
    }
}
