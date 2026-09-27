//! Проверки кириллического ISO-8859-5 mapping и его C1 control range.

use super::decode;

#[test]
fn decodes_cyrillic_letters_and_symbols_without_windows_mapping() {
    assert_eq!(decode(b"\xd2\xe5\xf1\xf2"), "вхёђ");
    assert_eq!(decode(b"\xd0\xd1\xd2"), "абв");
    assert_eq!(decode(b"\xff"), "џ");
}

#[test]
fn preserves_ascii_and_c1_control_scalars() {
    let octets: Vec<u8> = (0x00_u8..=0x9f_u8).collect();
    let expected: String = octets.iter().copied().map(char::from).collect();
    assert_eq!(decode(&octets), expected);
}

#[test]
fn decodes_all_octets_in_the_byte_domain() {
    let octets: Vec<u8> = (0x00_u8..=0xff_u8).collect();
    assert_eq!(decode(&octets).chars().count(), 256);
}
