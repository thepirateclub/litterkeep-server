//! Проверки выбранных границ IDNA2008 derived-property таблицы.

use super::{DerivedProperty, derived_property};

/// Распространённые lowercase letters относятся к PVALID.
#[test]
fn recognizes_protocol_valid_letters() {
    for character in ['a', 'z', 'ü', '例'] {
        assert_eq!(derived_property(character), DerivedProperty::PValid);
    }
}

/// Join controls и зарегистрированные contextual punctuation не становятся PVALID.
#[test]
fn keeps_context_required_code_points_typed() {
    for character in ['\u{200c}', '\u{200d}'] {
        assert_eq!(derived_property(character), DerivedProperty::ContextJ);
    }
    for character in ['\u{00b7}', '\u{0375}', '\u{0660}', '\u{30fb}'] {
        assert_eq!(derived_property(character), DerivedProperty::ContextO);
    }
}

/// Таблица не разрешает uppercase, пробел или unassigned scalar автоматически.
#[test]
fn rejects_disallowed_and_unassigned_code_points() {
    for character in ['A', '_', ' ', '\u{0378}', '\u{1f600}'] {
        assert_eq!(
            derived_property(character),
            DerivedProperty::DisallowedOrUnassigned
        );
    }
}
