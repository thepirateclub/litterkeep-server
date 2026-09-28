//! Классификация присвоенных scalar values Unicode 17.0.0.

#[path = "unicode_17_ranges/plane_00_a.rs"]
mod plane_00_a;
#[path = "unicode_17_ranges/plane_00_b.rs"]
mod plane_00_b;
#[path = "unicode_17_ranges/plane_01_a.rs"]
mod plane_01_a;
#[path = "unicode_17_ranges/plane_01_b.rs"]
mod plane_01_b;
#[path = "unicode_17_ranges/plane_02.rs"]
mod plane_02;
#[path = "unicode_17_ranges/plane_03.rs"]
mod plane_03;
#[path = "unicode_17_ranges/plane_04.rs"]
mod plane_04;
#[path = "unicode_17_ranges/plane_05.rs"]
mod plane_05;
#[path = "unicode_17_ranges/plane_06.rs"]
mod plane_06;
#[path = "unicode_17_ranges/plane_07.rs"]
mod plane_07;
#[path = "unicode_17_ranges/plane_08.rs"]
mod plane_08;
#[path = "unicode_17_ranges/plane_09.rs"]
mod plane_09;
#[path = "unicode_17_ranges/plane_0A.rs"]
mod plane_0a;
#[path = "unicode_17_ranges/plane_0B.rs"]
mod plane_0b;
#[path = "unicode_17_ranges/plane_0C.rs"]
mod plane_0c;
#[path = "unicode_17_ranges/plane_0D.rs"]
mod plane_0d;
#[path = "unicode_17_ranges/plane_0E.rs"]
mod plane_0e;
#[path = "unicode_17_ranges/plane_0F.rs"]
mod plane_0f;
#[path = "unicode_17_ranges/plane_10.rs"]
mod plane_10;

/// Версия UCD, из которой воспроизводятся присвоенные/unassigned ranges.
pub(super) const UNICODE_DATA_VERSION: (u8, u8, u8) = (17, 0, 0);

/// Сборка запрещается, если стандартные Unicode-таблицы Rust сменили версию.
const _: () = match char::UNICODE_VERSION {
    UNICODE_DATA_VERSION => (),
    _ => panic!("Unicode UCD tables must match char::UNICODE_VERSION"),
};

/// Возвращает true только для scalar values, назначенных в Unicode 17.0.0.
pub(crate) fn is_assigned(character: char) -> bool {
    let code_point = u32::from(character);
    let ranges = unassigned_ranges(code_point);
    ranges
        .binary_search_by(|(start, end)| compare_code_point(code_point, *start, *end))
        .is_err()
}

/// Выбирает отсортированные непересекающиеся незанятые интервалы нужного блока.
fn unassigned_ranges(code_point: u32) -> &'static [(u32, u32)] {
    match code_point {
        0x00000..=0x07FFF => plane_00_a::RANGES,
        0x08000..=0x0FFFF => plane_00_b::RANGES,
        0x10000..=0x17FFF => plane_01_a::RANGES,
        0x18000..=0x1FFFF => plane_01_b::RANGES,
        0x20000..=0x2FFFF => plane_02::RANGES,
        0x30000..=0x3FFFF => plane_03::RANGES,
        0x40000..=0x4FFFF => plane_04::RANGES,
        0x50000..=0x5FFFF => plane_05::RANGES,
        0x60000..=0x6FFFF => plane_06::RANGES,
        0x70000..=0x7FFFF => plane_07::RANGES,
        0x80000..=0x8FFFF => plane_08::RANGES,
        0x90000..=0x9FFFF => plane_09::RANGES,
        0xA0000..=0xAFFFF => plane_0a::RANGES,
        0xB0000..=0xBFFFF => plane_0b::RANGES,
        0xC0000..=0xCFFFF => plane_0c::RANGES,
        0xD0000..=0xDFFFF => plane_0d::RANGES,
        0xE0000..=0xEFFFF => plane_0e::RANGES,
        0xF0000..=0xFFFFF => plane_0f::RANGES,
        0x100000..=0x10FFFF => plane_10::RANGES,
        _ => &[],
    }
}

/// Binary search ordering for one inclusive unassigned interval.
fn compare_code_point(code_point: u32, start: u32, end: u32) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (code_point.cmp(&start), code_point.cmp(&end)) {
        (Ordering::Less, _) => Ordering::Greater,
        (_, Ordering::Greater) => Ordering::Less,
        _ => Ordering::Equal,
    }
}

#[cfg(test)]
#[path = "unicode_17_tests.rs"]
mod tests;
