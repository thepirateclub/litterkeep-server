//! Тесты декодирования и обратной проверки RFC 5891 A-label.

use super::{UlabelError, decode};

/// Принимает канонический A-label и ASCII-регистр prefix/body.
#[test]
fn decodes_valid_alabel_case_insensitively() {
    assert_eq!(
        crate::smtp::punycode::decode("bcher-kva"),
        Ok("bücher".to_owned())
    );
    assert_eq!(
        super::super::ulabel::to_alabel("bücher"),
        Ok("xn--bcher-kva".to_owned())
    );
    assert_eq!(decode("xn--bcher-kva"), Ok("bücher".to_owned()));
    assert_eq!(decode("XN--BCHER-KVA"), Ok("bücher".to_owned()));
}

/// Не допускает пустой ACE payload, ASCII-only output и невалидный Punycode.
#[test]
fn rejects_invalid_alabel_encodings() {
    for label in ["xn--", "xn--abc", "xn--bcher-kv!", "other-label"] {
        assert_eq!(decode(label), Err(UlabelError::InvalidALabel), "{label}");
    }
}

/// A-label также обязан укладываться в DNS-ограничение wire label.
#[test]
fn rejects_overlong_alabel_before_decoding() {
    let label = format!("xn--{}", "a".repeat(60));
    assert_eq!(decode(&label), Err(UlabelError::EncodedLabelTooLong));
}
