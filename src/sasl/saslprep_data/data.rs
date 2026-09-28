//! Индекс Unicode 3.2 таблиц SASLprep.
#[path = "properties_index.rs"]
mod properties;
pub(super) const PROPERTIES: &[&[(u32, u32, u16)]] = properties::ENTRIES;
#[path = "nfkd_index.rs"]
mod nfkd;
pub(super) const NFKD: &[&[(u32, &[u32])]] = nfkd::ENTRIES;
#[path = "combining_index.rs"]
mod combining;
pub(super) const COMBINING: &[&[(u32, u32, u8)]] = combining::ENTRIES;
#[path = "composition_index.rs"]
mod composition;
pub(super) const COMPOSITION: &[&[(u32, u32, u32)]] = composition::ENTRIES;
