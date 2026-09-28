//! Проверки RFC 2231 коллизий и пустых extended-параметров.

use super::parameters;

/// RFC 2231 не разрешает regular и extended формы одного имени одновременно.
#[test]
fn rejects_regular_and_extended_parameter_name_collisions() {
    for value in [
        "text/plain; filename=plain.txt; filename*=UTF-8''extended.txt",
        "text/plain; filename=plain.txt; filename*0*=UTF-8''extended.txt",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value}");
    }
}

/// Порядок параметров незначим: regular форма не должна пропасть после extended.
#[test]
fn rejects_regular_parameter_after_extended_form() {
    for value in [
        "text/plain; filename*=UTF-8''extended.txt; filename=plain.txt",
        "text/plain; filename*0*=UTF-8''extended.txt; filename=plain.txt",
    ] {
        assert!(parameters::parse(value).is_err(), "accepted {value}");
    }
}

/// RFC 2231 допускает пустой payload после charset и language delimiters.
#[test]
fn accepts_empty_rfc2231_extended_payload() {
    let parsed =
        parameters::parse("text/plain; filename*=UTF-8''").expect("empty extended payload");

    assert_eq!(
        parsed.parameters.get("filename").map(String::as_str),
        Some("")
    );
}
