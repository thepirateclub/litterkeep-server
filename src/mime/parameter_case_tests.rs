//! Проверки регистра и уникальности имён MIME parameter.

use super::parameters;

/// RFC 2045 требует сопоставлять имена parameter без учёта регистра.
#[test]
fn parameter_names_are_case_insensitive() {
    let parsed = parameters::parse("text/plain; NAME=value").expect("uppercase attribute");

    assert_eq!(
        parsed.parameters.get("name").map(String::as_str),
        Some("value")
    );
}

/// Разные по регистру повторения являются одним MIME parameter.
#[test]
fn rejects_case_insensitive_duplicate_parameter_names() {
    assert!(parameters::parse("text/plain; NAME=one; name=two").is_err());
}
