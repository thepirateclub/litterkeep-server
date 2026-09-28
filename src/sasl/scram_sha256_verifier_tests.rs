//! Проверяет SCRAM-SHA-256 verifier по RFC 7677 и локальным границам.

use super::{MAX_PASSWORD_SIZE, ScramSha256Verifier};
use crate::mime::decode_base64;

/// Проверяет proof и signature полного RFC 7677 §3 примера.
#[test]
fn rfc_7677_example_verifies_client_proof_and_server_signature() {
    let salt = decode_base64(b"W22ZaJ0SNY7soEsUEjb6gQ==").expect("RFC salt");
    let verifier = ScramSha256Verifier::derive(b"pencil", &salt, 4096).expect("verifier");
    let auth_message = [
        b"n=user,r=rOprNGfwEbeRWgbNEkqO".as_slice(),
        b",r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,",
        b"s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096,",
        b"c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0",
    ]
    .concat();
    let proof = decode_base64(b"dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ=")
        .expect("published client proof");
    let signature = decode_base64(b"6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
        .expect("published server signature");

    assert!(verifier.verify_client_proof(&auth_message, &proof));
    assert_eq!(
        verifier.server_signature(&auth_message).as_slice(),
        signature
    );
}

/// Отвергает изменённый proof и неверную длину до crypto comparison.
#[test]
fn rejects_modified_or_wrong_length_client_proof() {
    let verifier =
        ScramSha256Verifier::derive(b"pencil", b"RFC salt", 4096).expect("valid verifier");
    let message = b"client-first,server-first,client-final";
    assert!(!verifier.verify_client_proof(message, &[0; 32]));
    assert!(!verifier.verify_client_proof(message, &[0; 31]));
}

/// SASLprep-equivalent пароли дают одинаковый SCRAM-SHA-256 verifier.
#[test]
fn applies_saslprep_before_key_derivation() {
    let compatibility = ScramSha256Verifier::derive("\u{00AA}".as_bytes(), b"salt", 4096)
        .expect("prepared password");
    let normalized = ScramSha256Verifier::derive(b"a", b"salt", 4096).expect("normalized password");
    assert_eq!(compatibility.stored_key(), normalized.stored_key());
    assert_eq!(compatibility.server_key(), normalized.server_key());
}

/// Невалидные параметры и пароли не запускают дорогой KDF.
#[test]
fn rejects_unbounded_or_invalid_verifier_inputs() {
    assert!(ScramSha256Verifier::derive(b"pencil", b"", 4096).is_none());
    assert!(ScramSha256Verifier::derive(b"pencil", b"salt", 4095).is_none());
    assert!(ScramSha256Verifier::derive(b"pencil", b"salt", 200_001).is_none());
    assert!(ScramSha256Verifier::derive(&[0xff], b"salt", 4096).is_none());
    assert!(ScramSha256Verifier::derive("bad\u{0007}password".as_bytes(), b"salt", 4096).is_none());
    let oversized = vec![b'a'; MAX_PASSWORD_SIZE + 1];
    assert!(ScramSha256Verifier::derive(&oversized, b"salt", 4096).is_none());
}
