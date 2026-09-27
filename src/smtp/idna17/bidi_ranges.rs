//! Индекс bidi ranges Unicode 17.
#[path = "bidi_ranges/bidi_00.rs"]
mod bidi_00;
#[path = "bidi_ranges/bidi_01.rs"]
mod bidi_01;
#[path = "bidi_ranges/bidi_02.rs"]
mod bidi_02;
#[path = "bidi_ranges/bidi_03.rs"]
mod bidi_03;

pub(super) const TABLES: &[&[(u32, u32, super::BidiClass)]] = &[
    bidi_00::RANGES,
    bidi_01::RANGES,
    bidi_02::RANGES,
    bidi_03::RANGES,
];
