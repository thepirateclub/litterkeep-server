//! Проверки KOI8-U Unicode mapping и IANA charset identifiers.

use super::super::charset::Charset;
use super::decode;

#[test]
fn maps_every_octet_using_the_rfc_2319_ukrainian_overrides() {
    let octets: Vec<u8> = (0_u8..=u8::MAX).collect();
    let russian = super::super::charset_koi8_r::decode(&octets);
    let expected: String = russian
        .chars()
        .zip(octets.iter().copied())
        .map(|(character, octet)| match octet {
            0xa4 => '\u{0454}',
            0xa6 => '\u{0456}',
            0xa7 => '\u{0457}',
            0xad => '\u{0491}',
            0xb4 => '\u{0404}',
            0xb6 => '\u{0406}',
            0xb7 => '\u{0407}',
            0xbd => '\u{0490}',
            _ => character,
        })
        .collect();

    assert_eq!(decode(&octets), expected);
}

#[test]
fn canonical_name_alias_and_ukrainian_mapping_are_supported() {
    assert_eq!(Charset::parse("KOI8-U"), Ok(Charset::Koi8U));
    assert_eq!(Charset::parse("csKOI8U"), Ok(Charset::Koi8U));
    assert_eq!(
        Charset::Koi8U.decode(b"\xb7\xd6\xc1\xcb"),
        Ok("Їжак".to_owned())
    );
}
