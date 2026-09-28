//! Проверяет обязательную грамматику имён ManageSieve по RFC 5804.

use super::valid_name;

/// Имена допускают строковые specials, slash и минимум 128 Unicode characters.
#[test]
fn accepts_quoted_specials_slashes_and_128_unicode_characters() {
    assert!(valid_name("review\"\\draft/final").is_ok());
    assert!(valid_name(&"🧪".repeat(128)).is_ok());
}

/// Имена не могут инъецировать строки ответа или обходить лимит RFC 5804.
#[test]
fn rejects_forbidden_net_unicode_and_names_over_128_characters() {
    for name in [
        "\0control",
        "line\rbreak",
        "line\nbreak",
        "delete\u{007f}",
        "c1\u{0085}",
        "line\u{2028}separator",
        "paragraph\u{2029}separator",
        "\u{0378}unassigned",
        "\u{feff}leading-bom",
    ] {
        assert!(
            valid_name(name).is_err(),
            "accepted forbidden name {name:?}"
        );
    }
    assert!(valid_name(&"a".repeat(129)).is_err());
}

/// BOM не запрещён внутри имени: RFC 5198 запрещает только начальную позицию.
#[test]
fn accepts_bom_after_the_first_scalar() {
    assert!(valid_name("x\u{feff}y").is_ok());
}
