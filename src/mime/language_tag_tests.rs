//! Регрессии RFC 5646 для языковых метаданных MIME RFC 2231.

use super::LanguageTag;

#[test]
fn accepts_bcp47_structural_profiles_and_preserves_original_case() {
    for value in [
        "en",
        "zh-Hant-TW",
        "cmn-Hans-CN",
        "en-u-ca-gregory",
        "en-a-aaa-b-ccc-x-local",
        "x-company-private",
        "i-klingon",
        "en-GB-oed",
    ] {
        let parsed = LanguageTag::parse(value).expect("well-formed language tag");
        assert_eq!(parsed.as_str(), value);
    }
}

#[test]
fn rejects_malformed_and_ambiguous_bcp47_sequences() {
    for value in [
        "",
        "en--US",
        "e",
        "english-abcdefghij",
        "en-u",
        "en-a-aaa-a-bbb",
        "en-1901-1901",
        "en_US",
        "en-x",
        "x",
        "русский",
    ] {
        assert!(LanguageTag::parse(value).is_err(), "accepted {value}");
    }
}
