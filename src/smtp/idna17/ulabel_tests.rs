//! Проверки RFC 5891 U-label, которым не нужны контекстные или bidi-таблицы.

use super::{UlabelError, to_alabel};

/// Официальные многоязычные векторы проходят PVALID, NFC и кодирование A-label.
#[test]
fn converts_normalized_unicode_labels_to_alabels() {
    for (label, expected) in [
        ("例え", "xn--r8jz45g"),
        ("日本語", "xn--wgv71a119e"),
        ("bücher", "xn--bcher-kva"),
    ] {
        assert_eq!(to_alabel(label), Ok(expected.to_owned()));
    }
}

/// Не-NFC, неверные дефисы и запрещённые кодовые точки отклоняются.
#[test]
fn rejects_non_nfc_and_structurally_invalid_labels() {
    assert_eq!(to_alabel("e\u{301}"), Err(UlabelError::NotNormalized));
    assert_eq!(to_alabel("-例"), Err(UlabelError::HyphenRestriction));
    assert_eq!(to_alabel("例-"), Err(UlabelError::HyphenRestriction));
    assert_eq!(to_alabel("ab--例"), Err(UlabelError::HyphenRestriction));
    assert_eq!(to_alabel("例_"), Err(UlabelError::NonPValidCodePoint));
}

/// Контекстные символы остаются запрещены до реализации правил RFC.
#[test]
fn reports_contextual_rules_as_unimplemented() {
    assert_eq!(
        to_alabel("a\u{200c}b"),
        Err(UlabelError::ContextRuleRequired)
    );
    assert_eq!(
        to_alabel("l\u{00b7}l"),
        Err(UlabelError::ContextRuleRequired)
    );
}

/// Предел DNS label проверяется после Punycode и добавления ACE-префикса.
#[test]
fn rejects_encoded_labels_over_dns_octet_limit() {
    let label = format!("{}例", "é".repeat(60));
    assert_eq!(to_alabel(&label), Err(UlabelError::EncodedLabelTooLong));
}
