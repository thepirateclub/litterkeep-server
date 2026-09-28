//! Исчерпывающее сравнение pointer slots с обеими EUC-JP decoder ветками.

use super::{JIS0208_POINTER_COUNT, JIS0212_POINTER_COUNT, TABLE, jis0208, jis0212};

#[test]
fn asset_shape_matches_both_pointer_indexes() {
    assert_eq!(TABLE.get(..4), Some(b"LKEJ".as_slice()));
    assert_eq!(
        TABLE.len(),
        12 + (JIS0208_POINTER_COUNT + JIS0212_POINTER_COUNT) * 4
    );
}

#[test]
fn every_jis0208_pointer_matches_its_wire_pair() {
    for pointer in 0..JIS0212_POINTER_COUNT {
        let lead = u8::try_from(pointer / 94).expect("lead") + 0xa1;
        let trail = u8::try_from(pointer % 94).expect("trail") + 0xa1;
        let bytes = [lead, trail];
        assert_eq!(
            super::super::charset_euc_jp::decode(&bytes).ok(),
            jis0208(pointer).map(|c| c.to_string())
        );
    }
}

#[test]
fn every_jis0212_pointer_matches_its_wire_sequence() {
    for pointer in 0..JIS0212_POINTER_COUNT {
        let lead = u8::try_from(pointer / 94).expect("lead") + 0xa1;
        let trail = u8::try_from(pointer % 94).expect("trail") + 0xa1;
        let bytes = [0x8f, lead, trail];
        assert_eq!(
            super::super::charset_euc_jp::decode(&bytes).ok(),
            jis0212(pointer).map(|c| c.to_string())
        );
    }
}
