//! Подмодули таблицы properties профиля SASLprep.
#[path = "properties_000.rs"]
mod properties_000;
#[path = "properties_001.rs"]
mod properties_001;
#[path = "properties_002.rs"]
mod properties_002;
#[path = "properties_003.rs"]
mod properties_003;
#[path = "properties_004.rs"]
mod properties_004;
pub(super) const ENTRIES: &[&[(u32, u32, u16)]] = &[
    properties_000::ENTRIES,
    properties_001::ENTRIES,
    properties_002::ENTRIES,
    properties_003::ENTRIES,
    properties_004::ENTRIES,
];
