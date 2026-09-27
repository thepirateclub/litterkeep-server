//! Проверки декодера quoted-printable по нормативным правилам RFC 2045.

use super::quoted_printable::decode;

/// RFC 2045 §6.7 иллюстрирует мягкий перенос, который не добавляет CRLF.
#[test]
fn decodes_rfc_soft_line_break_examples() {
    let examples = [
        (
            &b"Now's the time =\r\nfor all folk to come=\r\n to the aid of their country."[..],
            &b"Now's the time for all folk to come to the aid of their country."[..],
        ),
        (&b"=3D =0C =7E"[..], &b"= \x0c ~"[..]),
    ];

    for (encoded, expected) in examples {
        assert_eq!(decode(encoded).expect("RFC QP vector"), expected);
    }
}

/// Получатель обязан удалять транспортную padding, не затрагивая encoded WSP.
#[test]
fn removes_hard_line_padding_but_preserves_space_before_soft_break() {
    let encoded = b"kept=20 \t\r\nsoft \t=\r\nnext";
    assert_eq!(
        decode(encoded).expect("transport padding"),
        b"kept \r\nsoft \tnext"
    );
}

/// RFC допускает устойчивое декодирование длинной ошибочной входящей строки.
#[test]
fn decodes_overlong_incoming_line_without_truncating_octets() {
    let line = vec![b'a'; 77];
    assert_eq!(decode(&line).expect("robust overlong decoding"), line);
}

/// Все printable octets и обе hex-регистровые формы дают исходные байты.
#[test]
fn decodes_safe_octets_and_case_insensitive_hex_from_real_world_mail() {
    let encoded = b"!#$%&'()*+,-./0123456789:;<>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~=af=AF";
    let mut expected = b"!#$%&'()*+,-./0123456789:;<>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~".to_vec();
    expected.extend_from_slice(&[0xaf, 0xaf]);

    assert_eq!(
        decode(encoded).expect("printable and hexadecimal data"),
        expected
    );
}

/// Проверяет каждый возможный октет через uppercase и robust lowercase hex.
#[test]
fn decodes_the_complete_octet_range_in_both_hex_cases() {
    for octet in 0_u8..=u8::MAX {
        for encoded in [format!("={octet:02X}"), format!("={octet:02x}")] {
            assert_eq!(
                decode(encoded.as_bytes()),
                Ok(vec![octet]),
                "decoded octet {octet:02X} from {encoded}"
            );
        }
    }
}

/// Проверяет весь RFC 2045 §6.7 диапазон допустимого literal US-ASCII.
#[test]
fn accepts_every_rfc_literal_printable_octet() {
    for octet in (33_u8..=60).chain(62_u8..=126) {
        assert_eq!(decode(&[octet]), Ok(vec![octet]), "literal octet {octet}");
    }
}
