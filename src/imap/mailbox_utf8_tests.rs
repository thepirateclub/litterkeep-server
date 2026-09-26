//! Проверки точных запретов RFC 9755 для UTF-8 mailbox names.

use super::{MailboxNameError, validate};

/// Разрешает обычный Unicode и combining sequence без потери исходных scalar.
#[test]
fn preserves_valid_and_non_normalized_unicode() {
    assert_eq!(validate("Папка café"), Ok(()));
    assert_eq!(validate("e\u{0301}"), Ok(()));
}

/// Отвергает все запрещённые RFC 9755 диапазоны и разделители строк.
#[test]
fn rejects_forbidden_controls_and_line_separators() {
    for character in [
        '\u{0000}', '\u{001f}', '\u{007f}', '\u{0085}', '\u{009f}', '\u{2028}', '\u{2029}',
    ] {
        assert_eq!(
            validate(&format!("folder{character}name")),
            Err(MailboxNameError::ControlCharacter),
            "scalar U+{:04X}",
            character as u32
        );
    }
}

/// Запрещает BOM только в начале network text согласно RFC 5198.
#[test]
fn rejects_leading_bom_but_keeps_interior_scalar() {
    assert_eq!(
        validate("\u{feff}folder"),
        Err(MailboxNameError::LeadingByteOrderMark)
    );
    assert_eq!(validate("fol\u{feff}der"), Ok(()));
}

/// Не пропускает code points, отсутствующие в закреплённом Unicode 17.0.0.
#[test]
fn rejects_unassigned_code_points_across_planes() {
    for character in ['\u{0378}', '\u{1fffe}', '\u{10ffff}'] {
        assert_eq!(
            validate(&format!("folder{character}")),
            Err(MailboxNameError::UnassignedCodePoint),
            "scalar U+{:04X}",
            character as u32
        );
    }
}

/// Оставляет допустимыми старые, новые и частные назначенные scalar values.
#[test]
fn accepts_assigned_unicode_including_private_use() {
    for character in ['\u{2ebf0}', '\u{1f600}', '\u{e000}', '\u{10fffd}'] {
        assert_eq!(validate(&format!("folder{character}")), Ok(()));
    }
}
