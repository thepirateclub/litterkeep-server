//! Граничные проверки Script-таблицы CONTEXTO.

use super::{Script, script_of};

/// Таблица распознаёт каждый Script, используемый правилами IDNA.
#[test]
fn recognizes_contexto_script_boundaries() {
    for (character, expected) in [
        ('α', Script::Greek),
        ('א', Script::Hebrew),
        ('あ', Script::Hiragana),
        ('カ', Script::Katakana),
        ('漢', Script::Han),
    ] {
        assert_eq!(script_of(character), Some(expected));
    }
}

/// Посторонний ASCII и Latin символы не дают ложного Script совпадения.
#[test]
fn leaves_unrelated_scripts_unclassified() {
    for character in ['a', '7', '\u{0378}'] {
        assert_eq!(script_of(character), None);
    }
}
