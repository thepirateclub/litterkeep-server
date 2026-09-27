//! Unicode 17 Canonical_Combining_Class=Virama lookup для IDNA CONTEXTJ.

#[path = "virama_ranges.rs"]
mod ranges;

/// Проверяет принадлежность code point к Unicode Virama property.
pub(super) fn is_virama(character: char) -> bool {
    let code_point = u32::from(character);
    ranges::RANGES
        .binary_search_by(|(start, end)| compare_code_point(code_point, *start, *end))
        .is_ok()
}

/// Сравнивает scalar value с одним включительным диапазоном.
fn compare_code_point(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "virama_tests.rs"]
mod tests;
