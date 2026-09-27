//! Тесты категорий combining marks для проверки первого символа RFC 5891.

use super::is_mark;

/// Находит непробельные, пробельные и охватывающие знаки, но не буквы.
#[test]
fn recognizes_all_unicode_mark_categories() {
    assert!(is_mark('\u{0301}'));
    assert!(is_mark('\u{093e}'));
    assert!(is_mark('\u{20dd}'));
    assert!(!is_mark('a'));
    assert!(!is_mark('א'));
}
