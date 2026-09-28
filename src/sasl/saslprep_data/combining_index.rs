//! Подмодули таблицы combining профиля SASLprep.
#[path = "combining_000.rs"]
mod combining_000;
pub(super) const ENTRIES: &[&[(u32, u32, u8)]] = &[combining_000::ENTRIES];
