//! Индекс небольших таблиц IDNA2008 derived-property ranges.
#[path = "ranges/ranges_00.rs"]
mod ranges_00;
#[path = "ranges/ranges_01.rs"]
mod ranges_01;
#[path = "ranges/ranges_02.rs"]
mod ranges_02;
#[path = "ranges/ranges_03.rs"]
mod ranges_03;
#[path = "ranges/ranges_04.rs"]
mod ranges_04;

pub(super) const TABLES: &[&[(u32, u32, super::DerivedProperty)]] = &[
    ranges_00::RANGES,
    ranges_01::RANGES,
    ranges_02::RANGES,
    ranges_03::RANGES,
    ranges_04::RANGES,
];
