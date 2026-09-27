//! Полные проверки Baltic Rim mapping ISO-8859-13.

use super::decode;

#[test]
fn maps_every_octet_to_the_unicode_reference_profile() {
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    let decoded = decode(&bytes).expect("all ISO-8859-13 octets are defined");
    let expected: String = (0..=u8::MAX)
        .map(|byte| match byte {
            0xa1 => '\u{201d}',
            0xa5 => '\u{201e}',
            0xa8 => '\u{00d8}',
            0xaa => '\u{0156}',
            0xaf => '\u{00c6}',
            0xb4 => '\u{201c}',
            0xb8 => '\u{00f8}',
            0xba => '\u{0157}',
            0xbf => '\u{00e6}',
            0xc0 => '\u{0104}',
            0xc1 => '\u{012e}',
            0xc2 => '\u{0100}',
            0xc3 => '\u{0106}',
            0xc6 => '\u{0118}',
            0xc7 => '\u{0112}',
            0xc8 => '\u{010c}',
            0xca => '\u{0179}',
            0xcb => '\u{0116}',
            0xcc => '\u{0122}',
            0xcd => '\u{0136}',
            0xce => '\u{012a}',
            0xcf => '\u{013b}',
            0xd0 => '\u{0160}',
            0xd1 => '\u{0143}',
            0xd2 => '\u{0145}',
            0xd4 => '\u{014c}',
            0xd8 => '\u{0172}',
            0xd9 => '\u{0141}',
            0xda => '\u{015a}',
            0xdb => '\u{016a}',
            0xdd => '\u{017b}',
            0xde => '\u{017d}',
            0xe0 => '\u{0105}',
            0xe1 => '\u{012f}',
            0xe2 => '\u{0101}',
            0xe3 => '\u{0107}',
            0xe6 => '\u{0119}',
            0xe7 => '\u{0113}',
            0xe8 => '\u{010d}',
            0xea => '\u{017a}',
            0xeb => '\u{0117}',
            0xec => '\u{0123}',
            0xed => '\u{0137}',
            0xee => '\u{012b}',
            0xef => '\u{013c}',
            0xf0 => '\u{0161}',
            0xf1 => '\u{0144}',
            0xf2 => '\u{0146}',
            0xf4 => '\u{014d}',
            0xf8 => '\u{0173}',
            0xf9 => '\u{0142}',
            0xfa => '\u{015b}',
            0xfb => '\u{016b}',
            0xfd => '\u{017c}',
            0xfe => '\u{017e}',
            0xff => '\u{2019}',
            _ => char::from(byte),
        })
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn preserves_ascii_and_c1_controls() {
    let bytes: Vec<u8> = (0..=0x9f).collect();
    let expected: String = bytes.iter().copied().map(char::from).collect();
    assert_eq!(decode(&bytes), Ok(expected));
}

#[test]
fn decodes_lithuanian_place_name() {
    assert_eq!(decode(b"\xd0iauliai"), Ok("Šiauliai".to_owned()));
}
