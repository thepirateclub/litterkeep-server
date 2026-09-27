//! Тесты IDNA domain conversion, входящих A-label и domain-wide bidi.

use super::{UlabelError, is_valid_ascii_domain, to_alabel_domain};

/// Преобразует обычные, U-label и A-label компоненты в lowercase DNS form.
#[test]
fn converts_mixed_idna_domain_to_canonical_alabel_form() {
    assert_eq!(
        to_alabel_domain("bücher.Example"),
        Ok("xn--bcher-kva.example".to_owned())
    );
    assert_eq!(
        to_alabel_domain("XN--BCHER-KVA.Example"),
        Ok("xn--bcher-kva.example".to_owned())
    );
}

/// Отклоняет пустые метки, не-LDH ASCII и избыточную длину всего DNS имени.
#[test]
fn rejects_invalid_domain_structure_and_length() {
    assert_eq!(to_alabel_domain("example..test"), Err(UlabelError::Empty));
    assert_eq!(
        to_alabel_domain("-bad.test"),
        Err(UlabelError::InvalidAsciiLabel)
    );
    let too_long = format!(
        "{}.{}.{}.{}",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(63)
    );
    assert_eq!(to_alabel_domain(&too_long), Err(UlabelError::DomainTooLong));
}

/// При RTL-метке проверяются также соседние ASCII labels по RFC 5893.
#[test]
fn applies_bidi_rule_to_every_label_in_the_domain() {
    assert!(to_alabel_domain("mail.مثال").is_ok());
    assert_eq!(to_alabel_domain("123.مثال"), Err(UlabelError::BidiRule));
    assert_eq!(to_alabel_domain("١٢.example"), Err(UlabelError::BidiRule));
    assert!(to_alabel_domain("123.example").is_ok());
}

/// Проверяет A-label глубже, чем поверхностную LDH-грамматику ASCII.
#[test]
fn validates_incoming_ascii_domain_alabels() {
    assert!(is_valid_ascii_domain("mail.xn--bcher-kva"));
    assert!(!is_valid_ascii_domain("mail.مثال"));
    assert!(!is_valid_ascii_domain("mail.xn--"));
    assert!(!is_valid_ascii_domain("123.xn--mgbh0fb"));
}
