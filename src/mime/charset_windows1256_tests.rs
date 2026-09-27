//! Проверка каждой пары octet/scalar таблицы CP1256.

use super::decode;

#[test]
/// Сверяет входные octets с независимым арифметическим oracle.
fn every_octet_matches_the_vendor_mapping() {
    for byte in 0..=u8::MAX {
        let expected = match byte {
            0x80 => Ok('\u{20ac}'),
            0x81 => Ok('\u{067e}'),
            0x82 => Ok('\u{201a}'),
            0x83 => Ok('\u{0192}'),
            0x84 => Ok('\u{201e}'),
            0x85 => Ok('\u{2026}'),
            0x86 => Ok('\u{2020}'),
            0x87 => Ok('\u{2021}'),
            0x88 => Ok('\u{02c6}'),
            0x89 => Ok('\u{2030}'),
            0x8a => Ok('\u{0679}'),
            0x8b => Ok('\u{2039}'),
            0x8c => Ok('\u{0152}'),
            0x8d => Ok('\u{0686}'),
            0x8e => Ok('\u{0698}'),
            0x8f => Ok('\u{0688}'),
            0x90 => Ok('\u{06af}'),
            0x91 => Ok('\u{2018}'),
            0x92 => Ok('\u{2019}'),
            0x93 => Ok('\u{201c}'),
            0x94 => Ok('\u{201d}'),
            0x95 => Ok('\u{2022}'),
            0x96 => Ok('\u{2013}'),
            0x97 => Ok('\u{2014}'),
            0x98 => Ok('\u{06a9}'),
            0x99 => Ok('\u{2122}'),
            0x9a => Ok('\u{0691}'),
            0x9b => Ok('\u{203a}'),
            0x9c => Ok('\u{0153}'),
            0x9d => Ok('\u{200c}'),
            0x9e => Ok('\u{200d}'),
            0x9f => Ok('\u{06ba}'),
            0xa1 => Ok('\u{060c}'),
            0xaa => Ok('\u{06be}'),
            0xba => Ok('\u{061b}'),
            0xbf => Ok('\u{061f}'),
            0xc0 => Ok('\u{06c1}'),
            0xc1..=0xd6 => char::from_u32(0x0621 + u32::from(byte - 0xc1))
                .ok_or("invalid CP1256 oracle scalar"),
            0xd7 => Ok('×'),
            0xd8..=0xdb => char::from_u32(0x0637 + u32::from(byte - 0xd8))
                .ok_or("invalid CP1256 oracle scalar"),
            0xdc..=0xdf => char::from_u32(0x0640 + u32::from(byte - 0xdc))
                .ok_or("invalid CP1256 oracle scalar"),
            0xe1 => Ok('\u{0644}'),
            0xe3..=0xe6 => char::from_u32(0x0645 + u32::from(byte - 0xe3))
                .ok_or("invalid CP1256 oracle scalar"),
            0xec..=0xed => char::from_u32(0x0649 + u32::from(byte - 0xec))
                .ok_or("invalid CP1256 oracle scalar"),
            0xf0..=0xf3 => char::from_u32(0x064b + u32::from(byte - 0xf0))
                .ok_or("invalid CP1256 oracle scalar"),
            0xf5..=0xf6 => char::from_u32(0x064f + u32::from(byte - 0xf5))
                .ok_or("invalid CP1256 oracle scalar"),
            0xf8 => Ok('\u{0651}'),
            0xfa => Ok('\u{0652}'),
            0xfd => Ok('\u{200e}'),
            0xfe => Ok('\u{200f}'),
            0xff => Ok('\u{06d2}'),
            _ => Ok(char::from(byte)),
        };
        assert_eq!(
            decode(&[byte]).map(|text| text.chars().next()),
            expected.map(Some)
        );
    }
}

#[test]
/// Проверяет арабский образец и дополнительные Unicode-различия.
fn decodes_arabic_and_preserves_persian_joining_marks() {
    assert_eq!(
        decode(b"\xc7\xe1\xda\xd1\xc8\xed\xc9"),
        Ok("العربية".to_owned())
    );
    assert_eq!(
        decode(b"\x81\x9d\x9e\xfd\xfe"),
        Ok("پ\u{200c}\u{200d}\u{200e}\u{200f}".to_owned())
    );
}
