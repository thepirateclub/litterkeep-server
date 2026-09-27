//! Индекс Unicode 17 General_Category Mark ranges.
#[path = "mark_ranges/marks_00.rs"]
mod marks_00;
#[path = "mark_ranges/marks_01.rs"]
mod marks_01;

pub(super) const TABLES: &[&[(u32, u32)]] = &[marks_00::RANGES, marks_01::RANGES];
