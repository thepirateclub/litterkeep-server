//! Сквозной crypto known-answer test из SCRAM-SHA-1 RFC 5802 §5.1.
use super::super::hmac_sha1::authenticate;
use super::{MAX_PASSWORD_SIZE, ScramSha1Verifier};
use crate::mime::decode_base64;

/// Проверяет proof и server signature полного опубликованного SCRAM диалога.
#[test]
fn rfc_5802_example_derives_matching_client_proof_and_server_signature() {
    let salt = decode_base64(b"QSXCR+Q6sek8bf92").expect("RFC salt");
    let verifier = ScramSha1Verifier::derive(b"pencil", &salt, 4096).expect("valid verifier");
    let auth_message = [
        b"n=user,r=fyko+d2lbbFgONRv9qkxdawL".as_slice(),
        b",r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,s=QSXCR+Q6sek8bf92,i=4096",
        b",c=biws,r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j",
    ]
    .concat();
    let client_signature = authenticate(verifier.stored_key(), &auth_message);
    let salted_password = derive_for_test(b"pencil", &salt, 4096);
    let mut client_proof = authenticate(&salted_password, b"Client Key");
    for (octet, signature) in client_proof.iter_mut().zip(client_signature) {
        *octet ^= signature;
    }
    assert_eq!(
        client_proof.as_slice(),
        decode_base64(b"v0X8v3Bz2T0CJGbJQyF0X+HI4Ts=").expect("RFC client proof")
    );
    let server_signature = authenticate(verifier.server_key(), &auth_message);
    assert_eq!(
        server_signature.as_slice(),
        decode_base64(b"rmF9pqV8S7suAoZWja4dJRkFsKQ=").expect("RFC server signature")
    );
}

/// Получает SaltedPassword независимо от хранимой verifier структуры.
fn derive_for_test(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 20] {
    crate::sasl::pbkdf2_sha1::derive(password, salt, iterations).expect("RFC parameters")
}

/// Неверные числа не должны запускать дорогостоящий KDF.
#[test]
fn verifier_rejects_out_of_policy_salt_and_iteration_values() {
    assert!(ScramSha1Verifier::derive(b"pencil", b"", 4096).is_none());
    assert!(ScramSha1Verifier::derive(b"pencil", &[0; 16], 4095).is_none());
    assert!(ScramSha1Verifier::derive(b"pencil", &[0; 16], 200_001).is_none());
}

/// Вывод verifier нормализует пароль по SASLprep до PBKDF2.
#[test]
fn verifier_derives_identical_keys_for_saslprep_equivalent_passwords() {
    let compatibility_character = "\u{00AA}".as_bytes();
    let normalized_character = b"a";
    let first = ScramSha1Verifier::derive(compatibility_character, b"same salt", 4096)
        .expect("valid SASLprep password");
    let second = ScramSha1Verifier::derive(normalized_character, b"same salt", 4096)
        .expect("valid normalized password");
    assert_eq!(first.stored_key(), second.stored_key());
    assert_eq!(first.server_key(), second.server_key());
}

/// Невалидный UTF-8 и SASLprep-prohibited пароль не доходят до PBKDF2.
#[test]
fn verifier_rejects_invalid_encoding_and_prohibited_password() {
    assert!(ScramSha1Verifier::derive(&[0xFF], b"same salt", 4096).is_none());
    assert!(
        ScramSha1Verifier::derive("bad\u{0007}password".as_bytes(), b"same salt", 4096).is_none()
    );
}

/// Отклоняет слишком длинный UTF-8 input до Unicode normalization и KDF.
#[test]
fn verifier_rejects_password_over_local_octet_limit() {
    let oversized = vec![b'a'; MAX_PASSWORD_SIZE + 1];
    assert!(ScramSha1Verifier::derive(&oversized, b"same salt", 4096).is_none());
}
