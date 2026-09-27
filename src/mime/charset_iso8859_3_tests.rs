//! Контрольные проверки таблицы ISO-8859-3 и undefined positions.

use super::decode;

#[test]
fn decodes_latin3_letters_and_preserves_low_octets() {
    assert_eq!(decode(b"Afr\xa1ika"), Ok("AfrĦika".to_owned()));
    assert_eq!(
        decode(b"\x00\x7f\x80\x9f"),
        Ok("\0\x7f\u{0080}\u{009f}".to_owned())
    );
    assert_eq!(
        decode(b"\xa1\xa8\xa9\xc1\xce\xe1\xee"),
        Ok("Ħ¨İÁÎáî".to_owned())
    );
}

#[test]
fn rejects_each_undefined_octet_without_losing_neighbor_context() {
    for octet in [0xa5, 0xae, 0xbe, 0xc3, 0xd0, 0xe3, 0xf0] {
        assert!(
            decode(&[b'A', octet, b'B']).is_err(),
            "accepted {octet:#04x}"
        );
    }
}

#[test]
fn all_defined_upper_octets_have_scalar_mappings() {
    let mut defined = Vec::new();
    for octet in 0xa0_u8..=0xff_u8 {
        if ![0xa5, 0xae, 0xbe, 0xc3, 0xd0, 0xe3, 0xf0].contains(&octet) {
            defined.push(octet);
        }
    }
    assert_eq!(
        decode(&defined)
            .expect("defined assignments")
            .chars()
            .count(),
        89
    );
}
