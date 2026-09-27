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
fn iso_8859_4_uses_registered_name_and_decodes_baltic_text() {
    let charset = Charset::parse("ISO-8859-4").expect("registered MIME charset");
    assert_eq!(charset.decode(b"Riga\xa1"), Ok("RigaĄ".to_owned()));
    assert!(charset.decode(b"\xa5").is_err());
}

#[test]
fn iso_8859_5_uses_registered_name_and_preserves_cyrillic_controls() {
    let charset = Charset::parse("ISO-8859-5").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xd2\xe5\xf1\xf2"), Ok("вхёђ".to_owned()));
    assert_eq!(charset.decode(b"\x80"), Ok("\u{0080}".to_owned()));
}

#[test]
fn iso_8859_6_uses_registered_name_and_rejects_undefined_positions() {
    let charset = Charset::parse("ISO-8859-6").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"\xc7\xe4\xc7\xd3\xe4\xc7\xe5"),
        Ok("الاسلام".to_owned())
    );
    assert!(charset.decode(b"\xa1").is_err());
}

#[test]
fn iso_8859_7_uses_registered_name_and_decodes_greek() {
    let charset = Charset::parse("ISO-8859-7").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"\xc5\xeb\xeb\xdc\xe4\xe1"),
        Ok("Ελλάδα".to_owned())
    );
    assert!(charset.decode(b"\xd2").is_err());
}

#[test]
fn iso_8859_8_uses_registered_name_and_decodes_hebrew() {
    let charset = Charset::parse("ISO-8859-8").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xf9\xec\xe5\xed"), Ok("שלום".to_owned()));
    assert!(charset.decode(b"\xfb").is_err());
}

#[test]
fn iso_8859_9_decodes_turkish_letters_and_latin5_aliases() {
    let preferred = Charset::parse("ISO-8859-9").expect("registered MIME charset");
    let turkish = b"\xddstanbul, g\xfcne\xfe ve c\xf6z\xfc";
    assert_eq!(
        preferred.decode(turkish),
        Ok("İstanbul, güneş ve cözü".to_owned())
    );
    assert_eq!(Charset::parse("latin5"), Ok(Charset::Iso8859_9));
    assert_eq!(Charset::parse("l5"), Ok(Charset::Iso8859_9));
}

#[test]
fn iso_8859_10_decodes_nordic_text_and_registered_aliases() {
    let charset = Charset::parse("ISO-8859-10").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"Isl\xe1nd er fallegt"),
        Ok("Islánd er fallegt".to_owned())
    );
    assert_eq!(Charset::parse("latin6"), Ok(Charset::Iso8859_10));
    assert_eq!(Charset::parse("l6"), Ok(Charset::Iso8859_10));
}

#[test]
fn tis_620_decodes_thai_and_rejects_undefined_octets() {
    let charset = Charset::parse("TIS-620").expect("registered Thai charset");
    assert_eq!(charset.decode(b"\xa1\xd2\xc2"), Ok("กาย".to_owned()));
    assert_eq!(Charset::parse("ISO-8859-11"), Ok(Charset::Tis620));
    assert!(charset.decode(b"\xdb").is_err());
}

#[test]
fn iso_8859_13_decodes_baltic_text_and_registered_alias() {
    let charset = Charset::parse("ISO-8859-13").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xd0iauliai"), Ok("Šiauliai".to_owned()));
    assert_eq!(Charset::parse("csISO885913"), Ok(Charset::Iso8859_13));
}

#[test]
fn iso_8859_14_decodes_celtic_text_and_registered_aliases() {
    let charset = Charset::parse("ISO-8859-14").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xa1\xb2 Cymru"), Ok("ḂĠ Cymru".to_owned()));
    assert_eq!(Charset::parse("latin8"), Ok(Charset::Iso8859_14));
    assert_eq!(Charset::parse("l8"), Ok(Charset::Iso8859_14));
    assert_eq!(Charset::parse("iso-celtic"), Ok(Charset::Iso8859_14));
    assert_eq!(Charset::parse("csISO885914"), Ok(Charset::Iso8859_14));
}

#[test]
fn iso_8859_15_decodes_latin9_replacements_and_registered_aliases() {
    let charset = Charset::parse("ISO-8859-15").expect("registered MIME charset");
    assert_eq!(charset.decode(b"Prix: \xa4"), Ok("Prix: €".to_owned()));
    assert_eq!(
        charset.decode(b"\xa6\xa8 \xbc\xbd\xbe"),
        Ok("Šš ŒœŸ".to_owned())
    );
    assert_eq!(Charset::parse("latin-9"), Ok(Charset::Iso8859_15));
    assert_eq!(Charset::parse("ISO_8859-15"), Ok(Charset::Iso8859_15));
    assert_eq!(Charset::parse("csISO885915"), Ok(Charset::Iso8859_15));
}

#[test]
fn iso_8859_16_decodes_latin10_text_and_registered_aliases() {
    let charset = Charset::parse("ISO-8859-16").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xaa\xde \xba\xfe"), Ok("ȘȚ șț".to_owned()));
    assert_eq!(charset.decode(b"Euro: \xa4"), Ok("Euro: €".to_owned()));
    for label in [
        "ISO_8859-16:2001",
        "ISO_8859-16",
        "latin10",
        "l10",
        "iso-ir-226",
        "csISO885916",
    ] {
        assert_eq!(Charset::parse(label), Ok(Charset::Iso8859_16));
    }
}

#[test]
fn nonexistent_iso_8859_12_is_not_guessed() {
    assert_eq!(
        Charset::parse("iso-8859-12"),
        Err("unsupported MIME charset")
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
fn windows_1250_decodes_central_european_text_and_rejects_undefined_octets() {
    let charset = Charset::parse("WINDOWS-1250").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"P\xf8\xed\x9at\xed"),
        Ok("Příští".to_owned())
    );
    assert_eq!(
        charset.decode(b"Za\xbf\xf3\xb3\xe6"),
        Ok("Zażółć".to_owned())
    );
    assert_eq!(Charset::parse("csWindows1250"), Ok(Charset::Windows1250));
    assert!(Charset::parse("cp1250").is_err());
    for byte in [0x81, 0x83, 0x88, 0x90, 0x98] {
        assert!(
            charset.decode(&[byte]).is_err(),
            "undefined octet {byte:#04x}"
        );
    }
}

#[test]
fn windows_1253_decodes_greek_and_rejects_undefined_octets() {
    let charset = Charset::parse("WINDOWS-1253").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"\xca\xe1\xeb\xe7\xec\xdd\xf1\xe1"),
        Ok("Καλημέρα".to_owned())
    );
    assert_eq!(Charset::parse("csWindows1253"), Ok(Charset::Windows1253));
    assert!(Charset::parse("cp1253").is_err());
    for byte in [
        0x81, 0x88, 0x8a, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x98, 0x9a, 0x9c, 0x9d, 0x9e, 0x9f, 0xaa,
        0xd2, 0xff,
    ] {
        assert!(
            charset.decode(&[byte]).is_err(),
            "undefined octet {byte:#04x}"
        );
    }
}

#[test]
fn windows_1254_decodes_turkish_and_rejects_undefined_octets() {
    let charset = Charset::parse("WINDOWS-1254").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"\xddstanbul, \xddzmir, I\xf0d\xfdr"),
        Ok("İstanbul, İzmir, Iğdır".to_owned())
    );
    assert_eq!(Charset::parse("csWindows1254"), Ok(Charset::Windows1254));
    assert!(Charset::parse("cp1254").is_err());
    for byte in [0x81, 0x8d, 0x8e, 0x8f, 0x90, 0x9d, 0x9e] {
        assert!(
            charset.decode(&[byte]).is_err(),
            "undefined octet {byte:#04x}"
        );
    }
}

#[test]
fn windows_1255_decodes_hebrew_and_rejects_undefined_octets() {
    let charset = Charset::parse("WINDOWS-1255").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xf9\xec\xe5\xed"), Ok("שלום".to_owned()));
    assert_eq!(Charset::parse("csWindows1255"), Ok(Charset::Windows1255));
    assert!(Charset::parse("cp1255").is_err());
    for byte in [
        0x81, 0x8a, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x9a, 0x9c, 0x9d, 0x9e, 0x9f, 0xca, 0xd9, 0xda,
        0xdb, 0xdc, 0xdd, 0xde, 0xdf, 0xfb, 0xfc, 0xff,
    ] {
        assert!(
            charset.decode(&[byte]).is_err(),
            "undefined octet {byte:#04x}"
        );
    }
}

#[test]
fn windows_1256_decodes_arabic_and_accepts_only_registered_names() {
    let charset = Charset::parse("WINDOWS-1256").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"\xc7\xe1\xda\xd1\xc8\xed\xc9"),
        Ok("العربية".to_owned())
    );
    assert_eq!(Charset::parse("csWindows1256"), Ok(Charset::Windows1256));
    assert!(Charset::parse("cp1256").is_err());
}

#[test]
fn windows_1257_decodes_baltic_text_and_registered_alias() {
    let charset = Charset::parse("WINDOWS-1257").expect("registered MIME charset");
    assert_eq!(charset.decode(b"\xc0ietuva"), Ok("Ąietuva".to_owned()));
    assert_eq!(Charset::parse("csWindows1257"), Ok(Charset::Windows1257));
    assert!(Charset::parse("cp1257").is_err());
    for byte in [
        0x81, 0x83, 0x88, 0x8a, 0x8c, 0x90, 0x98, 0x9a, 0x9c, 0x9f, 0xa1, 0xa5,
    ] {
        assert!(
            charset.decode(&[byte]).is_err(),
            "undefined octet {byte:#04x}"
        );
    }
}

#[test]
fn windows_1258_preserves_vietnamese_combining_sequences_and_alias_policy() {
    let charset = Charset::parse("WINDOWS-1258").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"Ti\xea\xecng Vi\xea\xf2t"),
        Ok("Tiê\u{0301}ng Viê\u{0323}t".to_owned())
    );
    assert_eq!(Charset::parse("csWindows1258"), Ok(Charset::Windows1258));
    assert!(Charset::parse("cp1258").is_err());
}

#[test]
fn windows_874_decodes_thai_and_accepts_only_iana_names() {
    let charset = Charset::parse("WINDOWS-874").expect("registered MIME charset");
    assert_eq!(
        charset.decode(b"\xc0\xd2\xc9\xd2\xe4\xb7\xc2"),
        Ok("ภาษาไทย".to_owned())
    );
    assert_eq!(Charset::parse("csWindows874"), Ok(Charset::Windows874));
    assert!(Charset::parse("cp874").is_err());
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
