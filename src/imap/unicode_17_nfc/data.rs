//! Индекс generated Unicode 17.0.0 normalization shards.
#[path = "decomposition_000.rs"]
mod decomposition_000;
#[path = "decomposition_001.rs"]
mod decomposition_001;
#[path = "decomposition_002.rs"]
mod decomposition_002;
#[path = "decomposition_003.rs"]
mod decomposition_003;
#[path = "decomposition_004.rs"]
mod decomposition_004;
#[path = "decomposition_005.rs"]
mod decomposition_005;
#[path = "decomposition_006.rs"]
mod decomposition_006;
#[path = "decomposition_007.rs"]
mod decomposition_007;
#[path = "decomposition_008.rs"]
mod decomposition_008;
#[path = "decomposition_009.rs"]
mod decomposition_009;
#[path = "decomposition_010.rs"]
mod decomposition_010;
#[path = "decomposition_011.rs"]
mod decomposition_011;
#[path = "decomposition_012.rs"]
mod decomposition_012;
#[path = "decomposition_013.rs"]
mod decomposition_013;
pub(super) const DECOMPOSITION: &[&[(u32, &[u32])]] = &[
    decomposition_000::ENTRIES,
    decomposition_001::ENTRIES,
    decomposition_002::ENTRIES,
    decomposition_003::ENTRIES,
    decomposition_004::ENTRIES,
    decomposition_005::ENTRIES,
    decomposition_006::ENTRIES,
    decomposition_007::ENTRIES,
    decomposition_008::ENTRIES,
    decomposition_009::ENTRIES,
    decomposition_010::ENTRIES,
    decomposition_011::ENTRIES,
    decomposition_012::ENTRIES,
    decomposition_013::ENTRIES,
];
#[path = "combining_class_000.rs"]
mod combining_class_000;
#[path = "combining_class_001.rs"]
mod combining_class_001;
#[path = "combining_class_002.rs"]
mod combining_class_002;
pub(super) const COMBINING_CLASS: &[&[(u32, u32, u8)]] = &[
    combining_class_000::ENTRIES,
    combining_class_001::ENTRIES,
    combining_class_002::ENTRIES,
];
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
pub(super) const COMPOSITION: &[&[(u32, u32, u32)]] = &[
    composition_000::ENTRIES,
    composition_001::ENTRIES,
    composition_002::ENTRIES,
    composition_003::ENTRIES,
    composition_004::ENTRIES,
    composition_005::ENTRIES,
    composition_006::ENTRIES,
];
