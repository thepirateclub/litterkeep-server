//! Проверки полной таблицы ISO-8859-2 на польском тексте и границах.

use super::decode;

#[test]
fn decodes_polish_sample_and_full_upper_half() {
    let polish = b"Za\xbf\xf3\xb3\xe6 g\xea\xb6l\xb1 ja\xbc\xf1";
    assert_eq!(decode(polish), Ok("Zażółć gęślą jaźń".to_owned()));

    let decoded = decode(&(0xa0_u8..=0xff_u8).collect::<Vec<_>>())
        .expect("all ISO-8859-2 upper-half octets have mappings");
    assert_eq!(decoded.chars().count(), 96);
    assert!(decoded.contains('Ą'));
    assert!(decoded.contains('ż'));
}

#[test]
fn preserves_ascii_and_control_prefix() {
    let decoded =
        decode(&(0x00_u8..=0x9f_u8).collect::<Vec<_>>()).expect("ASCII and C1 prefix is defined");
    let expected: String = (0x00_u8..=0x9f_u8).map(char::from).collect();
    assert_eq!(decoded, expected);
}
