//! Unicode 17 joining type lookup для правила IDNA CONTEXTJ.

#[path = "joining_ranges.rs"]
mod ranges;

/// Классы joining, различаемые контекстным правилом U+200C.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum JoiningType {
    Left,
    Right,
    Dual,
    JoinCausing,
    Transparent,
    NonJoining,
}

/// Возвращает joining type символа; отсутствующая таблица означает NonJoining.
pub(super) fn joining_type(character: char) -> JoiningType {
    let code_point = u32::from(character);
    for table in ranges::TABLES {
        if let Ok(position) =
            table.binary_search_by(|(start, end, _)| compare_code_point(code_point, *start, *end))
        {
            return table
                .get(position)
                .map(|(_, _, joining)| *joining)
                .unwrap_or(JoiningType::NonJoining);
        }
    }
    JoiningType::NonJoining
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
#[path = "joining_property_tests.rs"]
mod tests;
