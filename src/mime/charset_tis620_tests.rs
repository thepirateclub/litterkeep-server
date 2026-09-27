//! Полное покрытие TIS-620 mapping и undefined positions.

use super::decode;

#[test]
fn decodes_thai_sample_and_currency_sign() {
    assert_eq!(decode(b"\xa1\xd2\xc2"), Ok("กาย".to_owned()));
    assert_eq!(decode(b"\xdf"), Ok("฿".to_owned()));
}

#[test]
fn preserves_ascii_and_c1_controls() {
    let bytes: Vec<u8> = (0..=0x9f).collect();
    let expected: String = bytes.iter().copied().map(char::from).collect();
    assert_eq!(decode(&bytes), Ok(expected));
}

#[test]
fn accepts_every_assigned_upper_octet_and_rejects_undefined_positions() {
    let assigned: Vec<u8> = (0xa0..=0xff)
        .filter(|byte| !matches!(byte, 0xdb..=0xde | 0xfc..=0xff))
        .collect();
    assert_eq!(
        decode(&assigned).expect("assigned octets").chars().count(),
        88
    );
    for byte in [0xdb, 0xdc, 0xdd, 0xde, 0xfc, 0xfd, 0xfe, 0xff] {
        assert!(decode(&[byte]).is_err(), "accepted {byte:#04x}");
    }
}
