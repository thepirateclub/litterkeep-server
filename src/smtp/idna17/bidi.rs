//! Проверка шести условий IDNA Bidi Rule из RFC 5893 §2.

use super::bidi_property::BidiClass;

/// Проверяет label, если Unicode Bidi_Class делает её частью Bidi domain.
pub(super) fn validate_label(label: &[char]) -> bool {
    let classes = label
        .iter()
        .copied()
        .map(super::bidi_property::bidi_class)
        .collect::<Vec<_>>();
    // Arabic digits alone do not activate the label-local RFC 5893 check.
    // Whole-domain activation requires the caller to consider sibling labels.
    let is_bidi_domain_label = classes
        .iter()
        .any(|class| matches!(class, BidiClass::RightToLeft | BidiClass::ArabicLetter));
    match is_bidi_domain_label {
        false => true,
        true => validate_bidi_domain_label(&classes),
    }
}

/// Применяет initial, allowed-set, ending и mixed-number условия RFC.
fn validate_bidi_domain_label(classes: &[BidiClass]) -> bool {
    match classes.first().copied() {
        Some(BidiClass::RightToLeft | BidiClass::ArabicLetter) => validate_rtl(classes),
        Some(BidiClass::LeftToRight) => validate_ltr(classes),
        _ => false,
    }
}

/// Проверяет RTL-ветвь: разрешённые классы, финал и непересечение EN/AN.
fn validate_rtl(classes: &[BidiClass]) -> bool {
    let allowed = classes.iter().copied().all(rtl_class_allowed);
    let ending = last_non_mark(classes).is_some_and(rtl_ending_allowed);
    let has_european = classes.contains(&BidiClass::EuropeanNumber);
    let has_arabic = classes.contains(&BidiClass::ArabicNumber);
    allowed && ending && !(has_european && has_arabic)
}

/// Проверяет единственный конечный класс RTL-метки после завершающих NSM.
fn rtl_ending_allowed(class: BidiClass) -> bool {
    matches!(
        class,
        BidiClass::RightToLeft
            | BidiClass::ArabicLetter
            | BidiClass::EuropeanNumber
            | BidiClass::ArabicNumber
    )
}

/// Проверяет разрешённый набор классов RTL-метки.
fn rtl_class_allowed(class: BidiClass) -> bool {
    matches!(
        class,
        BidiClass::RightToLeft
            | BidiClass::ArabicLetter
            | BidiClass::ArabicNumber
            | BidiClass::EuropeanNumber
            | BidiClass::EuropeanSeparator
            | BidiClass::CommonSeparator
            | BidiClass::EuropeanTerminator
            | BidiClass::OtherNeutral
            | BidiClass::BoundaryNeutral
            | BidiClass::NonspacingMark
    )
}

/// Проверяет LTR-ветвь: разрешённый набор и конечный L/EN после NSM.
fn validate_ltr(classes: &[BidiClass]) -> bool {
    let allowed = classes.iter().copied().all(ltr_class_allowed);
    let ending = last_non_mark(classes)
        .is_some_and(|class| matches!(class, BidiClass::LeftToRight | BidiClass::EuropeanNumber));
    allowed && ending
}

/// Проверяет разрешённый набор классов LTR-метки, находящейся в Bidi domain.
fn ltr_class_allowed(class: BidiClass) -> bool {
    matches!(
        class,
        BidiClass::LeftToRight
            | BidiClass::EuropeanNumber
            | BidiClass::EuropeanSeparator
            | BidiClass::CommonSeparator
            | BidiClass::EuropeanTerminator
            | BidiClass::OtherNeutral
            | BidiClass::BoundaryNeutral
            | BidiClass::NonspacingMark
    )
}

/// Возвращает последний значимый класс, игнорируя конечную последовательность NSM.
fn last_non_mark(classes: &[BidiClass]) -> Option<BidiClass> {
    classes
        .iter()
        .rev()
        .copied()
        .find(|class| *class != BidiClass::NonspacingMark)
}

#[cfg(test)]
#[path = "bidi_tests.rs"]
mod tests;
