//! Сквозной crypto known-answer test из SCRAM-SHA-1 RFC 5802 §5.1.
use super::super::hmac_sha1::authenticate;
use super::ScramSha1Verifier;
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
