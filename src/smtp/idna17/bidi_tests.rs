//! Проверки RFC 5893 шести условий на искусственных class vectors.

use super::{BidiClass as Class, validate_label};

/// RTL sequence соблюдает класс начала/конца и завершение NSM.
#[test]
fn accepts_rtl_labels_with_valid_endings() {
    for classes in [
        vec![Class::ArabicLetter, Class::ArabicLetter],
        vec![
            Class::RightToLeft,
            Class::EuropeanNumber,
            Class::NonspacingMark,
        ],
        vec![Class::ArabicLetter, Class::ArabicNumber],
    ] {
        assert!(validate_label(&characters_for(&classes)));
    }
}

/// RTL label отвергает неверное начало, окончание, классы и смешение цифр.
#[test]
fn rejects_invalid_rtl_sequences() {
    for classes in [
        vec![Class::EuropeanNumber, Class::ArabicLetter],
        vec![Class::ArabicLetter, Class::EuropeanSeparator],
        vec![
            Class::ArabicLetter,
            Class::EuropeanNumber,
            Class::ArabicNumber,
        ],
        vec![Class::ArabicLetter, Class::LeftToRight],
    ] {
        assert!(!validate_label(&characters_for(&classes)));
    }
}

/// LTR label внутри Bidi domain отвергает RTL-классы и неверный финальный класс.
#[test]
fn validates_ltr_labels_when_the_label_contains_rtl_classes() {
    assert!(!validate_label(&['a', '1', 'ا']));
    assert!(!validate_label(&['a', 'ا']));
    assert!(!validate_label(&['a', '1', 'ا', '+']));
}

/// Чистый LTR label не получает требования RFC 5893 вне Bidi domain.
#[test]
fn leaves_non_bidi_labels_outside_the_rfc5893_rule() {
    assert!(validate_label(&['1', 'a']));
    assert!(validate_label(&['+', 'a']));
}

/// Строит безопасные тестовые scalar values для заданных bidi-классов.
fn characters_for(classes: &[Class]) -> Vec<char> {
    classes.iter().copied().map(character_for).collect()
}

/// Находит representative scalar каждого используемого RFC-класса.
fn character_for(class: Class) -> char {
    match class {
        Class::LeftToRight => 'a',
        Class::RightToLeft => 'א',
        Class::ArabicLetter => 'ا',
        Class::EuropeanNumber => '1',
        Class::EuropeanSeparator => '+',
        Class::EuropeanTerminator => '$',
        Class::ArabicNumber => '١',
        Class::CommonSeparator => ',',
        Class::NonspacingMark => '\u{0301}',
        Class::BoundaryNeutral => '\u{200c}',
        Class::ParagraphSeparator => '\u{2029}',
        Class::SegmentSeparator => '\u{0009}',
        Class::Whitespace => ' ',
        Class::OtherNeutral => '#',
        Class::LeftToRightEmbedding => '\u{202a}',
        Class::LeftToRightOverride => '\u{202d}',
        Class::RightToLeftEmbedding => '\u{202b}',
        Class::RightToLeftOverride => '\u{202e}',
        Class::PopDirectionalFormat => '\u{202c}',
        Class::LeftToRightIsolate => '\u{2066}',
        Class::RightToLeftIsolate => '\u{2067}',
        Class::FirstStrongIsolate => '\u{2068}',
        Class::PopDirectionalIsolate => '\u{2069}',
    }
}
