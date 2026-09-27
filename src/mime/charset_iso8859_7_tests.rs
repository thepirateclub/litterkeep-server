//! Регрессии греческих букв и undefined octets ISO-8859-7.

use super::decode;

#[test]
fn decodes_greek_word_euro_and_final_sigma() {
    assert_eq!(decode(b"\xc5\xeb\xeb\xdc\xe4\xe1"), Ok("Ελλάδα".to_owned()));
    assert_eq!(decode(b"\xa4\xc2\xff"), Err("undefined ISO-8859-7 octet"));
    assert_eq!(decode(b"\xa4"), Ok("€".to_owned()));
    assert_eq!(decode(b"\xf2"), Ok("ς".to_owned()));
}

#[test]
fn preserves_ascii_and_c1_control_range() {
    let bytes: Vec<u8> = (0x00_u8..=0x9f_u8).collect();
    let expected: String = bytes.iter().copied().map(char::from).collect();
    assert_eq!(decode(&bytes), Ok(expected));
}

#[test]
fn accepts_every_assigned_upper_octet_and_rejects_undefined_bytes() {
    let assigned: Vec<u8> = (0xa0_u8..=0xfe_u8)
        .filter(|byte| *byte != 0xae && *byte != 0xd2)
        .collect();
    assert_eq!(
        decode(&assigned)
            .expect("assigned Greek octets")
            .chars()
            .count(),
        93
    );
    for byte in [0xae, 0xd2, 0xff] {
        assert!(
            decode(&[byte]).is_err(),
            "accepted undefined byte {byte:#04x}"
        );
    }
}
