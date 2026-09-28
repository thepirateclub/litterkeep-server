//! Проверяет PBKDF2-HMAC-SHA-256 по RFC 7914 §11.

use super::derive;

/// Выводит lowercase hex для сравнения фиксированного KDF результата.
fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Первый 32-октетный PBKDF2 block совпадает с официальным 64-октетным vector.
#[test]
fn rfc_7914_pbkdf2_sha256_vector_matches() {
    let derived = derive(b"passwd", b"salt", 1).expect("one iteration");
    assert_eq!(
        hexadecimal(&derived),
        "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc"
    );
}

/// Нулевая стоимость не должна выглядеть допустимым выводом ключа.
#[test]
fn rejects_zero_iterations() {
    assert!(derive(b"password", b"salt", 0).is_none());
    assert!(derive(b"password", b"salt", super::MAX_ITERATIONS + 1).is_none());
}
