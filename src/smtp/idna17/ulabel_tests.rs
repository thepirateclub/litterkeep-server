//! Проверки RFC 5891 U-label и контекстных правил с Unicode 17 таблицами.

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

/// Join controls остаются запрещены до реализации всех правил CONTEXTJ.
#[test]
fn reports_contextual_rules_as_unimplemented() {
    assert_eq!(
        to_alabel("a\u{200c}b"),
        Err(UlabelError::ContextRuleRequired)
    );
}

/// CONTEXTO проверяется по соседним символам и всей доменной метке.
#[test]
fn applies_contexto_rules_to_the_whole_label() {
    for label in ["l\u{00b7}l", "\u{0375}α", "א\u{05f3}", "カ\u{30fb}", "١٢"] {
        assert!(to_alabel(label).is_ok(), "{label:?}");
    }
    for label in ["a\u{00b7}b", "\u{0375}a", "\u{05f3}א", "a\u{30fb}", "١۲"] {
        assert_eq!(
            to_alabel(label),
            Err(UlabelError::InvalidContext),
            "{label:?}"
        );
    }
}

/// Предел DNS label проверяется после Punycode и добавления ACE-префикса.
#[test]
fn rejects_encoded_labels_over_dns_octet_limit() {
    let label = format!("{}例", "é".repeat(60));
    assert_eq!(to_alabel(&label), Err(UlabelError::EncodedLabelTooLong));
}
