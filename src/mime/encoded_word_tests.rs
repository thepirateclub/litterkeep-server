//! Регрессии RFC 2047 encoded-word и разрешённых header-контекстов.

use super::{decode, decode_header, decode_phrase, decode_with_metadata};

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
/// Проверяет, что charset IBM866 подключена к декодированию RFC 2047.
fn decodes_ibm866_encoded_word_from_legacy_russian_header() {
    assert_eq!(
        decode("=?IBM866?B?j+CooqXi?=").expect("IBM866 encoded-word"),
        "Привет"
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
fn decodes_rfc2231_encoded_word_language_metadata() {
    let decoded =
        decode_with_metadata("=?UTF-8*en-US?Q?caf=C3=A9?=").expect("RFC 2231 language extension");
    assert_eq!(decoded.text, "café");
    assert_eq!(
        decoded.language.as_ref().map(|tag| tag.as_str()),
        Some("en-US")
    );
    assert_eq!(decode("=?UTF-8?Q?caf=C3=A9?="), Ok("café".to_owned()));
}

#[test]
fn rejects_malformed_rfc2231_encoded_word_language_tags() {
    for word in [
        "=?UTF-8*?Q?text?=",
        "=?UTF-8*en--US?Q?text?=",
        "=?UTF-8*en-a-aaa-a-bbb?Q?text?=",
    ] {
        assert!(decode_with_metadata(word).is_err(), "accepted {word}");
        assert_eq!(decode_header(word), Ok(word.to_owned()));
    }
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
fn treats_unicode_whitespace_as_phrase_data_not_encoded_word_separator() {
    assert_eq!(decode_phrase("Alice\u{00a0}North"), "Alice\u{00a0}North");
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

/// Malformed starters не должны заставлять decoder искать delimiter до конца поля.
#[test]
fn preserves_large_run_of_oversized_encoded_word_candidates() {
    let malformed = format!("=?{}tail?=", "x=?".repeat(32_768));
    assert_eq!(decode_header(&malformed), Ok(malformed));
}

/// Ограниченный lookahead всё равно находит следующее корректное отдельное слово.
#[test]
fn decodes_valid_word_after_oversized_malformed_candidate() {
    let malformed = format!("=?{}", "x".repeat(80));
    let value = format!("{malformed} =?UTF-8?Q?ok?=");
    assert_eq!(decode_header(&value), Ok(format!("{malformed} ok")));
}
