//! Проверки Unicode 17 Canonical_Combining_Class=Virama lookup.

use super::is_virama;

/// Virama marks имеют класс 9, а обычные combining marks вектором не выбраны.
#[test]
fn distinguishes_virama_from_other_code_points() {
    assert!(is_virama('\u{094d}'));
    assert!(is_virama('\u{09cd}'));
    assert!(!is_virama('\u{093f}'));
    assert!(!is_virama('क'));
}
