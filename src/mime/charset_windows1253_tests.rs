//! Полный пооктетный oracle для Unicode vendor mapping CP1253.

use super::decode;

#[test]
fn every_octet_matches_the_vendor_reference_and_undefined_set() {
    for byte in 0..=u8::MAX {
        let expected = match byte {
            0x81 | 0x88 | 0x8a | 0x8c..=0x90 | 0x98 | 0x9a | 0x9c..=0x9f | 0xaa | 0xd2 | 0xff => {
                Err("undefined Windows-1253 octet")
            }
            0x80 => Ok('\u{20ac}'),
            0x82 => Ok('\u{201a}'),
            0x83 => Ok('\u{0192}'),
            0x84 => Ok('\u{201e}'),
            0x85 => Ok('\u{2026}'),
            0x86 => Ok('\u{2020}'),
            0x87 => Ok('\u{2021}'),
            0x89 => Ok('\u{2030}'),
            0x8b => Ok('\u{2039}'),
            0x91 => Ok('\u{2018}'),
            0x92 => Ok('\u{2019}'),
            0x93 => Ok('\u{201c}'),
            0x94 => Ok('\u{201d}'),
            0x95 => Ok('\u{2022}'),
            0x96 => Ok('\u{2013}'),
            0x97 => Ok('\u{2014}'),
            0x99 => Ok('\u{2122}'),
            0x9b => Ok('\u{203a}'),
            0xa1 => Ok('\u{0385}'),
            0xa2 => Ok('\u{0386}'),
            0xaf => Ok('\u{2015}'),
            0xb4 => Ok('\u{0384}'),
            0xb8 => Ok('\u{0388}'),
            0xb9 => Ok('\u{0389}'),
            0xba => Ok('\u{038a}'),
            0xbc => Ok('\u{038c}'),
            0xbe => Ok('\u{038e}'),
            0xbf => Ok('\u{038f}'),
            0xc0 => Ok('\u{0390}'),
            0xc1..=0xd1 => char::from_u32(0x0391 + u32::from(byte - 0xc1))
                .ok_or("invalid Windows-1253 oracle scalar"),
            0xd3..=0xdb => char::from_u32(0x03a3 + u32::from(byte - 0xd3))
                .ok_or("invalid Windows-1253 oracle scalar"),
            0xdc..=0xdf => char::from_u32(0x03ac + u32::from(byte - 0xdc))
                .ok_or("invalid Windows-1253 oracle scalar"),
            0xe0 => Ok('\u{03b0}'),
            0xe1..=0xf1 => char::from_u32(0x03b1 + u32::from(byte - 0xe1))
                .ok_or("invalid Windows-1253 oracle scalar"),
            0xf2 => Ok('\u{03c2}'),
            0xf3..=0xfe => char::from_u32(0x03c3 + u32::from(byte - 0xf3))
                .ok_or("invalid Windows-1253 oracle scalar"),
            _ => Ok(char::from(byte)),
        };
        assert_eq!(
            decode(&[byte]).map(|text| text.chars().next()),
            expected.map(Some)
        );
    }
}

#[test]
fn decodes_greek_text_and_registered_mime_names() {
    assert_eq!(
        decode(b"\xca\xe1\xeb\xe7\xec\xdd\xf1\xe1"),
        Ok("Καλημέρα".to_owned())
    );
}
