//! Exhaustive проверки всех pointer slots и структуры Big5 lookup asset.

use super::{POINTER_COUNT, TABLE, code_point};

#[test]
fn asset_header_and_dense_pointer_region_have_expected_size() {
    assert_eq!(TABLE.get(..4), Some(b"LKBG".as_slice()));
    assert_eq!(TABLE.len(), 8 + POINTER_COUNT * 4);
    let size_bytes = TABLE.get(4..8).expect("pointer count header");
    assert_eq!(
        u32::from_le_bytes(size_bytes.try_into().expect("four bytes")),
        19_782
    );
}

#[test]
fn every_pointer_matches_the_decoder_including_unmapped_slots() {
    for pointer in 0..POINTER_COUNT {
        let lead = u8::try_from(pointer / 157).expect("lead range") + 0x81;
        let slot = u8::try_from(pointer % 157).expect("trail range");
        let trail = match slot {
            0..=62 => slot + 0x40,
            _ => slot + 0x62,
        };
        let pair = [lead, trail];
        let expected = match pointer {
            1_133 => Some("Ê\u{0304}".to_owned()),
            1_135 => Some("Ê\u{030c}".to_owned()),
            1_164 => Some("ê\u{0304}".to_owned()),
            1_166 => Some("ê\u{030c}".to_owned()),
            _ => code_point(pointer).map(|character| character.to_string()),
        };
        assert_eq!(
            super::super::charset_big5::decode(&pair).ok(),
            expected,
            "pointer {pointer}"
        );
    }
    assert_eq!(code_point(POINTER_COUNT), None);
}
