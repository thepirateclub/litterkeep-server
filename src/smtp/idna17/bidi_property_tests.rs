//! Представительные свойства Bidi_Class из официальной Unicode 17 таблицы.

use super::{BidiClass, bidi_class};

/// Таблица распознаёт арабские, европейские, combining и default-L классы.
#[test]
fn classifies_known_unicode_bidi_values() {
    assert_eq!(bidi_class('ا'), BidiClass::ArabicLetter);
    assert_eq!(bidi_class('א'), BidiClass::RightToLeft);
    assert_eq!(bidi_class('١'), BidiClass::ArabicNumber);
    assert_eq!(bidi_class('1'), BidiClass::EuropeanNumber);
    assert_eq!(bidi_class('\u{0301}'), BidiClass::NonspacingMark);
    assert_eq!(bidi_class('a'), BidiClass::LeftToRight);
}
