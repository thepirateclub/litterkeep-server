//! Независимые проверки Microsoft CP1255 для каждого возможного octet.

use super::decode;

#[test]
/// Сверяет полный диапазон с отдельным нормативным oracle.
fn every_octet_matches_the_vendor_mapping() {
    for byte in 0..=u8::MAX {
        let expected = match byte {
            0x81
            | 0x8a
            | 0x8c..=0x90
            | 0x9a
            | 0x9c..=0x9f
            | 0xca
            | 0xd9..=0xdf
            | 0xfb..=0xfc
            | 0xff => Err("undefined Windows-1255 octet"),
            0x80 => Ok('\u{20ac}'),
            0x82 => Ok('\u{201a}'),
            0x83 => Ok('\u{0192}'),
            0x84 => Ok('\u{201e}'),
            0x85 => Ok('\u{2026}'),
            0x86 => Ok('\u{2020}'),
            0x87 => Ok('\u{2021}'),
            0x88 => Ok('\u{02c6}'),
            0x89 => Ok('\u{2030}'),
            0x8b => Ok('\u{2039}'),
            0x91 => Ok('\u{2018}'),
            0x92 => Ok('\u{2019}'),
            0x93 => Ok('\u{201c}'),
            0x94 => Ok('\u{201d}'),
            0x95 => Ok('\u{2022}'),
            0x96 => Ok('\u{2013}'),
            0x97 => Ok('\u{2014}'),
            0x98 => Ok('\u{02dc}'),
            0x99 => Ok('\u{2122}'),
            0x9b => Ok('\u{203a}'),
            0xa4 => Ok('\u{20aa}'),
            0xaa => Ok('\u{00d7}'),
            0xba => Ok('\u{00f7}'),
            0xc0..=0xc9 => char::from_u32(0x05b0 + u32::from(byte - 0xc0))
                .ok_or("invalid Windows-1255 oracle scalar"),
            0xcb..=0xd3 => char::from_u32(0x05bb + u32::from(byte - 0xcb))
                .ok_or("invalid Windows-1255 oracle scalar"),
            0xd4..=0xd8 => char::from_u32(0x05f0 + u32::from(byte - 0xd4))
                .ok_or("invalid Windows-1255 oracle scalar"),
            0xe0..=0xfa => char::from_u32(0x05d0 + u32::from(byte - 0xe0))
                .ok_or("invalid Windows-1255 oracle scalar"),
            0xfd => Ok('\u{200e}'),
            0xfe => Ok('\u{200f}'),
            _ => Ok(char::from(byte)),
        };
        assert_eq!(
            decode(&[byte]).map(|text| text.chars().next()),
            expected.map(Some)
        );
    }
}

#[test]
/// Проверяет Hebrew, новые израильские шекели и RTL marks.
fn decodes_hebrew_text_and_directional_octets() {
    assert_eq!(decode(b"\xf9\xec\xe5\xed"), Ok("שלום".to_owned()));
    assert_eq!(decode(b"\xa4"), Ok("₪".to_owned()));
    assert_eq!(decode(b"\xfd\xfe"), Ok("\u{200e}\u{200f}".to_owned()));
}
