//! Проверки зарегистрированных MIME charset и отказа на неверных octets.

use super::Charset;

#[test]
fn profiles_are_explicit_and_strict() {
    assert_eq!(Charset::Utf8.decode("ёж".as_bytes()).expect("utf8"), "ёж");
    assert_eq!(
        Charset::Iso8859_1.decode(b"caf\xe9").expect("latin1"),
        "café"
    );
    assert!(Charset::UsAscii.decode(b"\xff").is_err());
    assert_eq!(
        Charset::parse("windows-1251")
            .expect("cp1251")
            .decode(b"\xcf\xf0\xe8\xe2\xe5\xf2"),
        Ok("Привет".to_owned())
    );
    assert!(Charset::Windows1251.decode(b"\x98").is_err());
}

#[test]
fn iso_8859_2_uses_its_complete_registered_mapping() {
    let charset = Charset::parse("ISO-8859-2").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"Za\xbf\xf3\xb3\xe6 g\xea\xb6l\xb1 ja\xbc\xf1"),
        Ok("Zażółć gęślą jaźń".to_owned())
    );
    assert!(charset.decode(b"\x80").is_ok());
}

#[test]
fn iso_8859_3_uses_registered_name_and_rejects_undefined_octets() {
    let charset = Charset::parse("ISO-8859-3").expect("registered MIME charset");
    assert_eq!(charset.decode(b"Afr\xa1ika"), Ok("AfrĦika".to_owned()));
    assert!(charset.decode(b"\xa5").is_err());
}

#[test]
fn iso_8859_profiles_support_only_the_required_ascii_subset() {
    for label in [
        "iso-8859-4",
        "iso-8859-5",
        "iso-8859-6",
        "iso-8859-7",
        "iso-8859-8",
        "iso-8859-9",
        "iso-8859-10",
        "iso-8859-11",
        "iso-8859-13",
        "iso-8859-14",
        "iso-8859-15",
        "iso-8859-16",
    ] {
        assert_eq!(
            Charset::parse(label)
                .expect("registered ISO-8859 profile")
                .decode(b"ASCII header"),
            Ok("ASCII header".to_owned()),
            "wrong ASCII subset behavior for {label}"
        );
    }
    assert!(Charset::parse("iso-8859-12").is_err());
    assert!(
        Charset::parse("iso-8859-4")
            .expect("ISO-8859-4 ASCII subset")
            .decode(b"caf\xe9")
            .is_err()
    );
}

#[test]
fn windows_1252_decodes_typographic_range_and_c1_controls() {
    let decoded = Charset::parse("WINDOWS-1252")
        .expect("registered MIME charset")
        .decode(&(0x80_u8..=0x9f_u8).collect::<Vec<_>>())
        .expect("complete Windows-1252 C1 range");

    assert_eq!(
        decoded,
        "€\u{0081}‚ƒ„…†‡ˆ‰Š‹Œ\u{008d}Ž\u{008f}\u{0090}‘’“”•–—˜™š›œ\u{009d}žŸ"
    );
    assert!(Charset::parse("cp1252").is_err());
}

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
