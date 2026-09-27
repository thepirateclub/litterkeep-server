//! Индекс таблиц joining ranges Unicode 17.
#[path = "joining_ranges/joining_00.rs"]
mod joining_00;
#[path = "joining_ranges/joining_01.rs"]
mod joining_01;
#[path = "joining_ranges/joining_02.rs"]
mod joining_02;

pub(super) const TABLES: &[&[(u32, u32, super::JoiningType)]] =
    &[joining_00::RANGES, joining_01::RANGES, joining_02::RANGES];
