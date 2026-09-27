//! Проверки полной таблицы Latin-9 по всем 256 значениям октета.

use super::decode;

#[test]
fn maps_every_octet_to_the_unicode_reference_profile() {
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    let decoded = decode(&bytes);
    let expected: String = (0..=u8::MAX)
        .map(|byte| match byte {
            0xa4 => '€',
            0xa6 => 'Š',
            0xa8 => 'š',
            0xb4 => 'Ž',
            0xb8 => 'ž',
            0xbc => 'Œ',
            0xbd => 'œ',
            0xbe => 'Ÿ',
            _ => char::from(byte),
        })
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn decodes_euro_and_latin9_replacements() {
    let bytes = [
        0xa4, b' ', 0xa6, 0xa8, b' ', 0xb4, 0xb8, b' ', 0xbc, 0xbd, 0xbe,
    ];
    assert_eq!(decode(&bytes), "€ Šš Žž ŒœŸ");
}
