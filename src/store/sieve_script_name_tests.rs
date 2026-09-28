//! Проверяет обязательную грамматику имён ManageSieve по RFC 5804.

use super::{codec, script_name};

/// Имена допускают строковые specials, slash и минимум 128 Unicode characters.
#[test]
fn accepts_quoted_specials_slashes_and_128_unicode_characters() {
    assert!(script_name::validate("review\"\\draft/final").is_ok());
    assert!(script_name::validate(&"🧪".repeat(128)).is_ok());
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
            script_name::validate(name).is_err(),
            "accepted forbidden name {name:?}"
        );
    }
    assert!(script_name::validate(&"a".repeat(129)).is_err());
}

/// BOM не запрещён внутри имени: RFC 5198 запрещает только начальную позицию.
#[test]
fn accepts_bom_after_the_first_scalar() {
    assert!(script_name::validate("x\u{feff}y").is_ok());
}

/// Канонически эквивалентные запросы разрешаются к одному стабильному ключу.
#[test]
fn canonicalizes_decomposed_name_to_nfc() {
    let normalized = script_name::canonical("Cafe\u{301}").expect("valid name");
    assert_eq!(normalized, "Café");
    assert_eq!(
        script_name::canonical(&normalized).expect("NFC name"),
        normalized
    );
}

/// Старый decomposed snapshot открывается с NFC-ключом без потери active state.
#[test]
fn migrates_legacy_decomposed_catalog_name() {
    let source = format!(
        "next:1\nactive:{}\nscript:{}:1:{}\n",
        hex("Cafe\u{301}"),
        hex("Cafe\u{301}"),
        hex("keep;")
    );
    let state = codec::decode(&source).expect("legacy state should migrate");
    assert!(state.scripts.contains_key("Café"));
    assert_eq!(state.active.as_deref(), Some("Café"));
    let reopened = codec::decode(&codec::encoded(&state)).expect("reopen");
    assert!(reopened.scripts.contains_key("Café"));
    assert_eq!(reopened.active.as_deref(), Some("Café"));
}

/// Старый snapshot с двумя одинаковыми NFC-именами нельзя молча схлопывать.
#[test]
fn rejects_legacy_catalog_nfc_collision() {
    let source = format!(
        "next:2\nactive:\nscript:{}:1:{}\nscript:{}:2:{}\n",
        hex("Café"),
        hex("keep;"),
        hex("Cafe\u{301}"),
        hex("discard;")
    );
    let error = codec::decode(&source).expect_err("collision must remain visible");
    assert!(error.to_string().contains("collision"));
}

fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
