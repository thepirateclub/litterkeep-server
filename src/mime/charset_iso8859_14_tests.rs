//! Проверки Celtic mapping ISO-8859-14 по полному диапазону байтов.

use super::decode;

#[test]
fn maps_every_octet_to_the_unicode_reference_profile() {
    let bytes: Vec<u8> = (0..=u8::MAX).collect();
    let decoded = decode(&bytes).expect("all ISO-8859-14 octets are defined");
    let expected: String = (0..=u8::MAX)
        .map(|byte| match byte {
            0xa1 => '\u{1e02}',
            0xa2 => '\u{1e03}',
            0xa4 => '\u{010a}',
            0xa5 => '\u{010b}',
            0xa6 => '\u{1e0a}',
            0xa8 => '\u{1e80}',
            0xaa => '\u{1e82}',
            0xab => '\u{1e0b}',
            0xac => '\u{1ef2}',
            0xaf => '\u{0178}',
            0xb0 => '\u{1e1e}',
            0xb1 => '\u{1e1f}',
            0xb2 => '\u{0120}',
            0xb3 => '\u{0121}',
            0xb4 => '\u{1e40}',
            0xb5 => '\u{1e41}',
            0xb7 => '\u{1e56}',
            0xb8 => '\u{1e81}',
            0xb9 => '\u{1e57}',
            0xba => '\u{1e83}',
            0xbb => '\u{1e60}',
            0xbc => '\u{1ef3}',
            0xbd => '\u{1e84}',
            0xbe => '\u{1e85}',
            0xbf => '\u{1e61}',
            0xd0 => '\u{0174}',
            0xd7 => '\u{1e6a}',
            0xde => '\u{0176}',
            0xf0 => '\u{0175}',
            0xf7 => '\u{1e6b}',
            0xfe => '\u{0177}',
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
fn decodes_celtic_letters_and_names() {
    assert_eq!(decode(b"\xa1\xb2 Cymru"), Ok("ḂĠ Cymru".to_owned()));
}
