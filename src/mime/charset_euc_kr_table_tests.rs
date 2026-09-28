//! Проверяет форму закреплённого EUC-KR table asset.

use super::{POINTER_COUNT, asset_is_well_formed, code_point};

#[test]
fn asset_has_expected_header_and_size() {
    assert!(asset_is_well_formed());
    assert_eq!(POINTER_COUNT, 23_940);
}

#[test]
fn lookup_rejects_out_of_range_and_unmapped_positions() {
    assert_eq!(code_point(POINTER_COUNT), None);
    assert_eq!(code_point(23_939), None);
}
