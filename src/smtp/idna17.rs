//! Derived IDNA2008 code point properties for the pinned Unicode 17.0.0 release.

#[path = "idna17/alabel.rs"]
mod alabel;
#[path = "idna17/bidi.rs"]
mod bidi;
#[path = "idna17/bidi_property.rs"]
mod bidi_property;
#[path = "idna17/domain.rs"]
mod domain;
#[path = "idna17/joining_property.rs"]
mod joining_property;
#[path = "idna17/mark_property.rs"]
mod mark_property;
#[path = "idna17/mark_ranges.rs"]
mod mark_ranges;
#[path = "idna17/ranges.rs"]
mod ranges;
#[path = "idna17/script_property.rs"]
mod script_property;
#[path = "idna17/ulabel.rs"]
mod ulabel;
#[path = "idna17/virama.rs"]
mod virama;

/// Класс символа по таблице IDNA2008 Derived Property Unicode 17.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedProperty {
    /// Символ допустим по RFC 5892, но label всё ещё требует структурных проверок.
    PValid,
    /// Символ требует contextual правила Joiner по RFC 5892/RFC 5891.
    ContextJ,
    /// Символ требует contextual правила Other по RFC 5892/RFC 5891.
    ContextO,
    /// Символ disallowed, unassigned либо отсутствует в таблице допустимых свойств.
    DisallowedOrUnassigned,
}

pub use domain::{is_valid_ascii_domain, to_alabel_domain};
pub use ulabel::{UlabelError, to_alabel};

/// Возвращает производное свойство символа без принятия решения о валидности label.
///
/// RFC 5892 отдельно предупреждает, что PVALID для одиночного code point
/// недостаточно для принятия всего label; вызывающая проверка обязана применить
/// hyphen, NFC, contextual и bidi правила.
pub fn derived_property(character: char) -> DerivedProperty {
    let code_point = u32::from(character);
    for table in ranges::TABLES {
        if let Ok(position) =
            table.binary_search_by(|(start, end, _)| compare_code_point(code_point, *start, *end))
        {
            return table
                .get(position)
                .map(|(_, _, property)| *property)
                .unwrap_or(DerivedProperty::DisallowedOrUnassigned);
        }
    }
    DerivedProperty::DisallowedOrUnassigned
}

/// Сравнивает scalar value с одним включительным диапазоном таблицы.
fn compare_code_point(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "idna17/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "idna17/conformance_tests.rs"]
mod conformance_tests;

#[cfg(test)]
#[path = "idna17/idna_test_v2.rs"]
mod idna_test_v2;
