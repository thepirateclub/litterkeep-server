//! Контрактные тесты имен IANA для явно поддерживаемых MIME-кодировок.

use super::Charset;

#[test]
/// Проверяет только перечисленные aliases существующих ISO-8859 decoders.
fn registered_iso_8859_aliases_resolve_to_their_profiles() {
    let cases = [
        ("iso_8859-1", Charset::Iso8859_1),
        ("latin1", Charset::Iso8859_1),
        ("l1", Charset::Iso8859_1),
        ("ibm819", Charset::Iso8859_1),
        ("cp819", Charset::Iso8859_1),
        ("csisolatin1", Charset::Iso8859_1),
        ("iso_8859-2", Charset::Iso8859_2),
        ("latin2", Charset::Iso8859_2),
        ("l2", Charset::Iso8859_2),
        ("csisolatin2", Charset::Iso8859_2),
        ("iso_8859-3", Charset::Iso8859_3),
        ("latin3", Charset::Iso8859_3),
        ("l3", Charset::Iso8859_3),
        ("csisolatin3", Charset::Iso8859_3),
        ("iso_8859-4", Charset::Iso8859_4),
        ("latin4", Charset::Iso8859_4),
        ("l4", Charset::Iso8859_4),
        ("csisolatin4", Charset::Iso8859_4),
        ("iso_8859-5", Charset::Iso8859_5),
        ("cyrillic", Charset::Iso8859_5),
        ("csisolatincyrillic", Charset::Iso8859_5),
        ("iso_8859-6", Charset::Iso8859_6),
        ("ecma-114", Charset::Iso8859_6),
        ("asmo-708", Charset::Iso8859_6),
        ("arabic", Charset::Iso8859_6),
        ("csisolatinarabic", Charset::Iso8859_6),
        ("iso_8859-7", Charset::Iso8859_7),
        ("elot_928", Charset::Iso8859_7),
        ("ecma-118", Charset::Iso8859_7),
        ("greek", Charset::Iso8859_7),
        ("greek8", Charset::Iso8859_7),
        ("csisolatingreek", Charset::Iso8859_7),
        ("iso_8859-8", Charset::Iso8859_8),
        ("hebrew", Charset::Iso8859_8),
        ("csisolatinhebrew", Charset::Iso8859_8),
        ("iso_8859-13", Charset::Iso8859_13),
    ];

    for (label, expected) in cases {
        assert_eq!(Charset::parse(label), Ok(expected), "{label}");
    }
}

#[test]
/// Проверяет Windows aliases реестра и отклонение незарегистрированного x-cp.
fn registered_windows_aliases_resolve_to_supported_profiles() {
    let cases = [
        ("cp1252", Charset::Windows1252),
        ("cswindows1251", Charset::Windows1251),
        ("cswindows1252", Charset::Windows1252),
        ("cswindows1255", Charset::Windows1255),
        ("cswindows1256", Charset::Windows1256),
    ];

    for (label, expected) in cases {
        assert_eq!(Charset::parse(label), Ok(expected), "{label}");
    }

    assert!(Charset::parse("x-cp1252").is_err());
}

#[test]
/// IANA alias добавляется без изменения byte mapping Windows-1250.
fn windows_1250_accepts_its_registered_cs_alias() {
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
    for byte in [0x81, 0x83, 0x88, 0x90, 0x98] {
        assert!(
            charset.decode(&[byte]).is_err(),
            "undefined octet {byte:#04x}"
        );
    }
}
