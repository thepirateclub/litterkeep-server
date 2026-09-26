//! Тесты версии Unicode и границ generated unassigned tables.

use super::{UNICODE_DATA_VERSION, is_assigned};

/// Таблица должна точно совпадать с Unicode properties текущей сборки Rust.
#[test]
fn unicode_table_matches_rust_toolchain_version() {
    assert_eq!(char::UNICODE_VERSION, UNICODE_DATA_VERSION);
}

/// Проверяет присвоенный/unassigned переход в каждом хранимом диапазоне plane.
#[test]
fn classifies_known_scalars_from_bmp_to_plane_sixteen() {
    for character in [
        '\u{0041}',
        '\u{0378}',
        '\u{2ebf0}',
        '\u{e000}',
        '\u{10fffd}',
        '\u{10ffff}',
    ] {
        match character {
            '\u{0378}' | '\u{10ffff}' => assert!(!is_assigned(character)),
            _ => assert!(is_assigned(character)),
        }
    }
}
