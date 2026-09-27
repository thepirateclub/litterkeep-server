//! Пооктетные проверки соответствия Microsoft CP1254 mapping.

use super::decode;

#[test]
/// Сверяет decoder независимо заданным результатом для каждого octet.
fn every_octet_matches_the_vendor_mapping() {
    for byte in 0..=u8::MAX {
        let expected = match byte {
            0x81 | 0x8d..=0x90 | 0x9d..=0x9e => Err("undefined Windows-1254 octet"),
            0x80 => Ok('€'),
            0x82 => Ok('‚'),
            0x83 => Ok('ƒ'),
            0x84 => Ok('„'),
            0x85 => Ok('…'),
            0x86 => Ok('†'),
            0x87 => Ok('‡'),
            0x88 => Ok('ˆ'),
            0x89 => Ok('‰'),
            0x8a => Ok('Š'),
            0x8b => Ok('‹'),
            0x8c => Ok('Œ'),
            0x91 => Ok('‘'),
            0x92 => Ok('’'),
            0x93 => Ok('“'),
            0x94 => Ok('”'),
            0x95 => Ok('•'),
            0x96 => Ok('–'),
            0x97 => Ok('—'),
            0x98 => Ok('˜'),
            0x99 => Ok('™'),
            0x9a => Ok('š'),
            0x9b => Ok('›'),
            0x9c => Ok('œ'),
            0x9f => Ok('Ÿ'),
            0xd0 => Ok('Ğ'),
            0xdd => Ok('İ'),
            0xde => Ok('Ş'),
            0xf0 => Ok('ğ'),
            0xfd => Ok('ı'),
            0xfe => Ok('ş'),
            _ => Ok(char::from(byte)),
        };
        assert_eq!(
            decode(&[byte]).map(|text| text.chars().next()),
            expected.map(Some)
        );
    }
}

#[test]
/// Проверяет турецкие замены на реальном словесном образце.
fn decodes_turkish_text_and_preserves_ascii() {
    assert_eq!(
        decode(b"\xddstanbul, \xddzmir, I\xf0d\xfdr"),
        Ok("İstanbul, İzmir, Iğdır".to_owned())
    );
}
