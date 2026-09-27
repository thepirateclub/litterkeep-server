//! Проверки Latin-10 mapping на полном диапазоне октетов.

use super::decode;

#[test]
fn maps_every_octet_to_the_unicode_reference_profile() {
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    let decoded = decode(&bytes);
    let expected: String = (0..=u8::MAX)
        .map(|byte| match byte {
            0xa1 => 'Ą',
            0xa2 => 'ą',
            0xa3 => 'Ł',
            0xa4 => '€',
            0xa5 => '„',
            0xa6 => 'Š',
            0xa8 => 'š',
            0xaa => 'Ș',
            0xac => 'Ź',
            0xae => 'ź',
            0xaf => 'Ż',
            0xb2 => 'Č',
            0xb3 => 'ł',
            0xb4 => 'Ž',
            0xb5 => '”',
            0xb8 => 'ž',
            0xb9 => 'č',
            0xba => 'ș',
            0xbc => 'Œ',
            0xbd => 'œ',
            0xbe => 'Ÿ',
            0xbf => 'ż',
            0xc3 => 'Ă',
            0xc5 => 'Ć',
            0xd0 => 'Đ',
            0xd1 => 'Ń',
            0xd5 => 'Ő',
            0xd7 => 'Ś',
            0xd8 => 'Ű',
            0xdd => 'Ę',
            0xde => 'Ț',
            0xe3 => 'ă',
            0xe5 => 'ć',
            0xf0 => 'đ',
            0xf1 => 'ń',
            0xf5 => 'ő',
            0xf7 => 'ś',
            0xf8 => 'ű',
            0xfd => 'ę',
            0xfe => 'ț',
            _ => char::from(byte),
        })
        .collect();
    assert_eq!(decoded, expected);
}

#[test]
fn preserves_ascii_and_c1_controls() {
    let bytes: Vec<u8> = (0..=0x9f).collect();
    let expected: String = bytes.iter().copied().map(char::from).collect();
    assert_eq!(decode(&bytes), expected);
}
