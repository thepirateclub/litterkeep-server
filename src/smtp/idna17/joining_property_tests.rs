//! Проверки joining type таблицы, используемой правилами ZWNJ.

use super::{JoiningType, joining_type};

/// Данные Unicode 17 различают Arabic joining и transparent marks.
#[test]
fn looks_up_joining_classes_and_default_non_joining() {
    assert_eq!(joining_type('ب'), JoiningType::Dual);
    assert_eq!(joining_type('ا'), JoiningType::Right);
    assert_eq!(joining_type('\u{064e}'), JoiningType::Transparent);
    assert_eq!(joining_type('a'), JoiningType::NonJoining);
    assert_eq!(joining_type('\u{0640}'), JoiningType::JoinCausing);
}
