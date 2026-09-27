//! Проверки Hebrew mapping ISO-8859-8 и undefined octets.

use super::decode;

#[test]
fn decodes_hebrew_word_and_direction_marks() {
    assert_eq!(decode(b"\xf9\xec\xe5\xed"), Ok("שלום".to_owned()));
    assert_eq!(decode(b"\xfd\xfe"), Ok("\u{200e}\u{200f}".to_owned()));
}

#[test]
fn preserves_ascii_and_c1_controls() {
    let bytes: Vec<u8> = (0x00_u8..=0x9f_u8).collect();
    let expected: String = bytes.iter().copied().map(char::from).collect();
    assert_eq!(decode(&bytes), Ok(expected));
}

#[test]
fn rejects_undefined_high_octets_and_accepts_assigned_ones() {
    let assigned: Vec<u8> = (0xa0_u8..=0xfe_u8)
        .filter(|byte| {
            ![
                0xa1, 0xbf, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xcb,
                0xcc, 0xcd, 0xce, 0xcf, 0xd0, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9,
                0xda, 0xdb, 0xdc, 0xdd, 0xde, 0xfb, 0xfc,
            ]
            .contains(byte)
        })
        .collect();
    assert_eq!(
        decode(&assigned)
            .expect("assigned Hebrew octets")
            .chars()
            .count(),
        60
    );
    for byte in [0xa1, 0xbf, 0xc0, 0xd0, 0xfb, 0xfc, 0xff] {
        assert!(
            decode(&[byte]).is_err(),
            "accepted undefined byte {byte:#04x}"
        );
    }
}
