//! Проверки размеров и нормативных опорных точек GB18030 tables.

use super::{TABLES, four_byte_code_point, two_byte_code_point};

#[test]
fn checked_asset_has_the_generator_declared_shape() {
    assert_eq!(TABLES.get(..4), Some(b"LKGK".as_slice()));
    assert_eq!(TABLES.len(), 12 + (23_940 * 4) + (207 * 8));
}

#[test]
fn every_two_byte_pointer_is_addressable_with_valid_scalars_or_gap_sentinel() {
    for pointer in 0..23_940_usize {
        let lead = u8::try_from(pointer / 190).expect("lead offset") + 0x81;
        let slot = u8::try_from(pointer % 190).expect("trail offset");
        let trail = match slot {
            0..=62 => slot + 0x40,
            _ => slot + 0x41,
        };
        let pair = [lead, trail];
        match two_byte_code_point(pointer) {
            Some(character) => assert_eq!(
                super::super::charset_gb18030::decode(&pair),
                Ok(character.to_string())
            ),
            None => assert!(super::super::charset_gb18030::decode(&pair).is_err()),
        }
    }
    assert_eq!(two_byte_code_point(23_940), None);
}

#[test]
fn four_byte_ranges_keep_standard_boundaries_and_supplementary_scalars() {
    assert_eq!(four_byte_code_point(0), Some('\u{0080}'));
    assert_eq!(four_byte_code_point(7_457), Some('\u{e7c7}'));
    assert_eq!(four_byte_code_point(39_419), Some('\u{ffff}'));
    assert_eq!(four_byte_code_point(39_420), None);
    assert_eq!(four_byte_code_point(189_000), Some('\u{10000}'));
    assert_eq!(four_byte_code_point(1_237_575), Some('\u{10ffff}'));
    assert_eq!(four_byte_code_point(1_237_576), None);
}
