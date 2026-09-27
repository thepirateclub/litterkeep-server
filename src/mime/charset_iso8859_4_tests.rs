//! Проверки полной таблицы ISO-8859-4 и отсутствующих назначений.

use super::decode;

#[test]
fn decodes_baltic_letters_and_control_prefix() {
    assert_eq!(decode(b"Riga\xa1"), Ok("RigaĄ".to_owned()));
    assert_eq!(
        decode(b"\x00\x7f\x80\x9f\xa1\xa2\xc1\xe1"),
        Ok("\0\x7f\u{0080}\u{009f}ĄĸÁá".to_owned())
    );
}

#[test]
fn rejects_each_undefined_octet() {
    for octet in [0xa5, 0xae, 0xbe, 0xc3, 0xd0, 0xe3, 0xf0] {
        assert!(
            decode(&[b'A', octet, b'B']).is_err(),
            "accepted {octet:#04x}"
        );
    }
}

#[test]
fn all_assigned_upper_octets_are_decodable() {
    let assigned: Vec<u8> = (0xa0_u8..=0xff_u8)
        .filter(|octet| ![0xa5, 0xae, 0xbe, 0xc3, 0xd0, 0xe3, 0xf0].contains(octet))
        .collect();
    assert_eq!(
        decode(&assigned)
            .expect("assigned ISO-8859-4 bytes")
            .chars()
            .count(),
        89
    );
}
