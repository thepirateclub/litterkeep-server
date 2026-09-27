//! Полный octet oracle для отдельного Thai Windows-874 mapping.

use super::decode;

#[test]
/// Сверяет каждый octet по собственной таблице reference assignments.
fn every_octet_matches_the_vendor_mapping() {
    for byte in 0..=u8::MAX {
        assert_eq!(
            decode(&[byte]).ok().and_then(|text| text.chars().next()),
            reference(byte),
            "octet {byte:#04x}"
        );
    }
}

/// Возвращает ожидаемое назначение, не вызывая production decoder.
fn reference(byte: u8) -> Option<char> {
    match byte {
        0x00..=0x7f => Some(char::from(byte)),
        0x80 => Some('€'),
        0x85 => Some('…'),
        0x91 => Some('‘'),
        0x92 => Some('’'),
        0x93 => Some('“'),
        0x94 => Some('”'),
        0x95 => Some('•'),
        0x96 => Some('–'),
        0x97 => Some('—'),
        0xa0 => Some('\u{00a0}'),
        0xa1..=0xda => char::from_u32(0x0e01 + u32::from(byte - 0xa1)),
        0xdf => Some('\u{0e3f}'),
        0xe0..=0xfb => char::from_u32(0x0e40 + u32::from(byte - 0xe0)),
        _ => None,
    }
}

#[test]
/// Проверяет тайское слово и distinguishing differences от TIS-620.
fn decodes_thai_text_and_rejects_undefined_octets() {
    assert_eq!(
        decode(b"\xc0\xd2\xc9\xd2\xe4\xb7\xc2"),
        Ok("ภาษาไทย".to_owned())
    );
    assert_eq!(decode(b"\xdf"), Ok("฿".to_owned()));
    for byte in [
        0x81, 0x82, 0x83, 0x84, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90,
        0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f, 0xdb, 0xdc, 0xdd, 0xde, 0xfc, 0xfd, 0xfe,
        0xff,
    ] {
        assert!(decode(&[byte]).is_err(), "undefined octet {byte:#04x}");
    }
}
