//! Подмодули таблицы composition профиля SASLprep.
#[path = "composition_000.rs"]
mod composition_000;
#[path = "composition_001.rs"]
mod composition_001;
#[path = "composition_002.rs"]
mod composition_002;
#[path = "composition_003.rs"]
mod composition_003;
#[path = "composition_004.rs"]
mod composition_004;
#[path = "composition_005.rs"]
mod composition_005;
#[path = "composition_006.rs"]
mod composition_006;
pub(super) const ENTRIES: &[&[(u32, u32, u32)]] = &[
    composition_000::ENTRIES,
    composition_001::ENTRIES,
    composition_002::ENTRIES,
    composition_003::ENTRIES,
    composition_004::ENTRIES,
    composition_005::ENTRIES,
    composition_006::ENTRIES,
];
