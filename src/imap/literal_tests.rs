//! Проверяет синтаксис typed literal-маркеров.

use super::{Marker, Mode, parse};

#[test]
fn distinguishes_synchronizing_and_non_synchronizing_forms() {
    assert_eq!(
        parse("{42}"),
        Some(Marker {
            size: 42,
            mode: Mode::Synchronizing,
        })
    );
    assert_eq!(
        parse("{4096+}"),
        Some(Marker {
            size: 4096,
            mode: Mode::NonSynchronizing,
        })
    );
}

#[test]
fn rejects_non_decimal_or_ambiguous_markers() {
    for marker in ["{}", "{+}", "{-1}", "{1++}", "{1 +}", "{1}x"] {
        assert_eq!(parse(marker), None, "{marker:?}");
    }
}
