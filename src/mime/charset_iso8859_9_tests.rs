//! Полная проверка таблицы ISO-8859-9 и её турецких замен.

use super::decode;

#[test]
fn maps_every_octet_to_the_unicode_reference_profile() {
    let decoded = decode(&(0_u8..=u8::MAX).collect::<Vec<_>>());
    let expected: String = (0_u8..=u8::MAX)
        .map(|byte| match byte {
            0xd0 => '\u{011e}',
            0xdd => '\u{0130}',
            0xde => '\u{015e}',
            0xf0 => '\u{011f}',
            0xfd => '\u{0131}',
            0xfe => '\u{015f}',
            _ => char::from(byte),
        })
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn decodes_turkish_sample_and_preserves_controls() {
    assert_eq!(decode(b"\xddstanbul, g\xfcne\xfe"), "İstanbul, güneş");
    assert_eq!(decode(b"\x80\x9f"), "\u{0080}\u{009f}");
}
