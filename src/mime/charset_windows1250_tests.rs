//! Полный byte-level oracle для зарегистрированной таблицы Windows-1250.

use super::decode;

#[test]
fn every_octet_matches_the_unicode_vendor_mapping() {
    for byte in 0..=u8::MAX {
        let expected = match byte {
            0x81 | 0x83 | 0x88 | 0x90 | 0x98 => Err("undefined Windows-1250 octet"),
            0x80 => Ok('€'),
            0x82 => Ok('‚'),
            0x84 => Ok('„'),
            0x85 => Ok('…'),
            0x86 => Ok('†'),
            0x87 => Ok('‡'),
            0x89 => Ok('‰'),
            0x8a => Ok('Š'),
            0x8b => Ok('‹'),
            0x8c => Ok('Ś'),
            0x8d => Ok('Ť'),
            0x8e => Ok('Ž'),
            0x8f => Ok('Ź'),
            0x91 => Ok('‘'),
            0x92 => Ok('’'),
            0x93 => Ok('“'),
            0x94 => Ok('”'),
            0x95 => Ok('•'),
            0x96 => Ok('–'),
            0x97 => Ok('—'),
            0x99 => Ok('™'),
            0x9a => Ok('š'),
            0x9b => Ok('›'),
            0x9c => Ok('ś'),
            0x9d => Ok('ť'),
            0x9e => Ok('ž'),
            0x9f => Ok('ź'),
            0xa1 => Ok('\u{02c7}'),
            0xa2 => Ok('\u{02d8}'),
            0xa3 => Ok('Ł'),
            0xa5 => Ok('Ą'),
            0xaa => Ok('Ş'),
            0xaf => Ok('Ż'),
            0xb2 => Ok('\u{02db}'),
            0xb3 => Ok('ł'),
            0xb9 => Ok('ą'),
            0xba => Ok('ş'),
            0xbc => Ok('Ľ'),
            0xbd => Ok('\u{02dd}'),
            0xbe => Ok('ľ'),
            0xbf => Ok('ż'),
            0xc0 => Ok('Ŕ'),
            0xc3 => Ok('Ă'),
            0xc5 => Ok('Ĺ'),
            0xc6 => Ok('Ć'),
            0xc8 => Ok('Č'),
            0xca => Ok('Ę'),
            0xcc => Ok('Ě'),
            0xcf => Ok('Ď'),
            0xd0 => Ok('Đ'),
            0xd1 => Ok('Ń'),
            0xd2 => Ok('Ň'),
            0xd5 => Ok('Ő'),
            0xd8 => Ok('Ř'),
            0xd9 => Ok('Ů'),
            0xdb => Ok('Ű'),
            0xde => Ok('Ţ'),
            0xe0 => Ok('ŕ'),
            0xe3 => Ok('ă'),
            0xe5 => Ok('ĺ'),
            0xe6 => Ok('ć'),
            0xe8 => Ok('č'),
            0xea => Ok('ę'),
            0xec => Ok('ě'),
            0xef => Ok('ď'),
            0xf0 => Ok('đ'),
            0xf1 => Ok('ń'),
            0xf2 => Ok('ň'),
            0xf5 => Ok('ő'),
            0xf8 => Ok('ř'),
            0xf9 => Ok('ů'),
            0xfb => Ok('ű'),
            0xfe => Ok('ţ'),
            0xff => Ok('\u{02d9}'),
            _ => Ok(char::from(byte)),
        };
        assert_eq!(
            decode(&[byte]).map(|text| text.chars().next()),
            expected.map(Some)
        );
    }
}

#[test]
fn decodes_czech_polish_and_central_european_samples() {
    assert_eq!(decode(b"P\xf8\xed\x9at\xed"), Ok("Příští".to_owned()));
    assert_eq!(decode(b"Za\xbf\xf3\xb3\xe6"), Ok("Zażółć".to_owned()));
    assert_eq!(decode(b"\x8c\x9a \x8e\x9e"), Ok("Śš Žž".to_owned()));
}
