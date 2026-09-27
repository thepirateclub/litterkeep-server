//! Минимальные свойства Script для контекстных IDNA2008 правил.

#[path = "script_ranges.rs"]
mod ranges;

#[cfg(test)]
#[path = "script_property_tests.rs"]
mod tests;

/// Unicode Script, используемый правилами CONTEXTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Script {
    Greek,
    Hebrew,
    Hiragana,
    Katakana,
    Han,
}

/// Ищет Script символа в официальных Unicode 17 диапазонах.
pub(super) fn script_of(character: char) -> Option<Script> {
    let code_point = u32::from(character);
    ranges::RANGES
        .binary_search_by(|(start, end, _)| compare_code_point(code_point, *start, *end))
        .ok()
        .and_then(|position| ranges::RANGES.get(position))
        .map(|(_, _, script)| *script)
}

/// Сортирует scalar value относительно включительного диапазона.
fn compare_code_point(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}
