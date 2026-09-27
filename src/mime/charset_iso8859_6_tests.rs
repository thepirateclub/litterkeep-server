//! Проверки Arabic mappings, C1 controls и undefined ISO-8859-6 позиций.

use super::decode;

#[test]
fn decodes_arabic_letters_and_punctuation() {
    assert_eq!(
        decode(b"\xc7\xe4\xc7\xd3\xe4\xc7\xe5"),
        Ok("الاسلام".to_owned())
    );
    assert_eq!(decode(b"\xac\xbb\xbf"), Ok("،؛؟".to_owned()));
}

#[test]
fn preserves_ascii_and_c1_controls() {
    let octets: Vec<u8> = (0x00_u8..=0x9f_u8).collect();
    let expected: String = octets.iter().copied().map(char::from).collect();
    assert_eq!(decode(&octets), Ok(expected));
}

#[test]
fn rejects_every_undefined_high_octet() {
    for octet in [
        0xa1, 0xa2, 0xa3, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xae, 0xaf, 0xb0, 0xb1, 0xb2,
        0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbc, 0xbd, 0xbe, 0xc0, 0xdb, 0xdc, 0xdd,
        0xde, 0xdf, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd, 0xfe, 0xff,
    ] {
        assert!(
            decode(&[b'A', octet, b'B']).is_err(),
            "accepted {octet:#04x}"
        );
    }
}
