//! Проверка Unicode 17 General_Category Mark для RFC 5891.

/// Определяет категории Mn, Mc и Me с двоичным поиском по таблице ranges.
pub(super) fn is_mark(character: char) -> bool {
    let code_point = u32::from(character);
    super::mark_ranges::TABLES.iter().any(|table| {
        table
            .binary_search_by(|(start, end)| compare(code_point, *start, *end))
            .is_ok()
    })
}

/// Сравнивает scalar value с включительным диапазоном.
fn compare(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "mark_property_tests.rs"]
mod tests;
