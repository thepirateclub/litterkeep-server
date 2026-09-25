//! Регрессии RFC 2047 encoded-word и разрешённых header-контекстов.

use super::{decode, decode_header, decode_phrase};

#[test]
fn decodes_base64_utf8_and_q_latin1() {
    assert_eq!(
        decode("=?UTF-8?B?0J/RgNC40LLQtdGC?=").expect("base64"),
        "Привет"
    );
    assert_eq!(
        decode("=?ISO-8859-1?Q?caf=E9_file?=").expect("q"),
        "café file"
    );
}

#[test]
fn decodes_utf16be_encoded_word_without_external_codec() {
    assert_eq!(
        decode("=?UTF-16BE?B?AEgAaQ==?=").expect("UTF-16BE encoded-word"),
        "Hi"
    );
}

#[test]
fn rejects_unknown_charset_and_bad_q_escape() {
    assert_eq!(
        decode("=?windows-1251?Q?=CF=F0=E8=E2=E5=F2?=").expect("cp1251"),
        "Привет"
    );
    assert!(decode("=?UTF-8?Q?bad=ZZ?=").is_err());
}

#[test]
fn rejects_empty_and_non_printable_encoded_text() {
    for word in [
        "=?UTF-8?B??=",
        "=?UTF-8?Q??=",
        "=?UTF-8?Q?white space?=",
        "=?UTF-8?Q?tab\tinside?=",
        "=?UTF-8?Q?\u{0001}?=",
        "=?UTF-8?Q?\u{00e9}?=",
    ] {
        assert!(decode(word).is_err(), "accepted invalid word {word:?}");
    }
    assert_eq!(
        decode("=?UTF-8?Q?control=01?=").expect("encoded octet"),
        "control\u{0001}"
    );
}

#[test]
fn invalid_word_remains_visible_in_header_projection() {
    let word = "=?UTF-8?Q?tab\tinside?=";
    assert_eq!(decode_header(word).expect("ordinary header text"), word);
}

#[test]
fn phrase_q_encoding_uses_its_restricted_character_set() {
    let invalid_phrase_word = "=?UTF-8?Q?Smith.Jones?=";
    assert_eq!(decode(invalid_phrase_word), Ok("Smith.Jones".to_owned()));
    assert_eq!(decode_phrase(invalid_phrase_word), invalid_phrase_word);
    assert_eq!(
        decode_phrase("=?UTF-8?Q?Smith=2EJones!_*+-/?="),
        "Smith.Jones! *+-/"
    );
    assert_eq!(decode_phrase("=?UTF-8?B?U21pdGguSm9uZXM=?="), "Smith.Jones");
}

#[test]
fn decodes_adjacent_words_and_preserves_plain_prefix() {
    let value = "Re: =?UTF-8?B?0J/RgNC40LLQtdGC?= =?ISO-8859-1?Q?_caf=E9?=";
    assert_eq!(decode_header(value).expect("header"), "Re: Привет café");
}

#[test]
fn header_recognizes_only_complete_lws_delimited_words() {
    let word = "=?UTF-8?Q?caf=C3=A9?=";
    let embedded = format!("subject{word}tail");
    let suffix = format!("{word}tail");
    let bounded = format!("subject \t{word} next");
    let nested = format!("=?{word}");

    assert_eq!(decode_header(&embedded), Ok(embedded));
    assert_eq!(decode_header(&suffix), Ok(suffix));
    assert_eq!(
        decode_header(&bounded),
        Ok("subject \tcafé next".to_owned())
    );
    assert_eq!(decode_header(&nested), Ok(nested));
}
