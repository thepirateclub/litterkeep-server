//! Проверяет UTF-16 labels, порядок байтов, surrogate pairs и MIME suitability.

use super::Charset;

#[test]
fn utf16_handles_registered_byte_orders_bom_and_supplementary_characters() {
    assert_eq!(
        Charset::parse("UTF-16")
            .expect("generic UTF-16")
            .decode(b"\xfe\xff\x00H\x00i"),
        Ok("Hi".to_owned())
    );
    assert_eq!(
        Charset::parse("utf-16le")
            .expect("little-endian UTF-16")
            .decode(b"\x3d\xd8\x00\xde"),
        Ok("😀".to_owned())
    );
    assert_eq!(
        Charset::parse("utf-16be")
            .expect("big-endian UTF-16")
            .decode(b"\xd8\x3d\xde\x00"),
        Ok("😀".to_owned())
    );
}

#[test]
fn utf16_rejects_odd_octets_invalid_surrogates_and_opposite_bom() {
    let generic = Charset::parse("utf-16").expect("generic UTF-16");
    let big_endian = Charset::parse("utf-16be").expect("UTF-16BE");
    let little_endian = Charset::parse("utf-16le").expect("UTF-16LE");

    assert!(generic.decode(b"").is_err());
    assert!(generic.decode(b"\x00").is_err());
    assert!(big_endian.decode(b"\x00").is_err());
    assert_eq!(big_endian.decode(b""), Ok(String::new()));
    assert_eq!(little_endian.decode(b""), Ok(String::new()));
    assert!(big_endian.decode(b"\xd8\x00").is_err());
    assert!(big_endian.decode(b"\xff\xfe").is_err());
    assert!(little_endian.decode(b"\xfe\xff").is_err());
}

#[test]
fn utf16_labels_are_not_suitable_for_mime_text_subtypes() {
    for label in ["utf-16", "utf-16be", "utf-16le"] {
        let charset = Charset::parse(label).expect("registered UTF-16 label");
        assert!(!charset.is_suitable_for_text_mime());
    }
}
