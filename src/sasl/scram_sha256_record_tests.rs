//! Проверяет долговечность и строгий parser SCRAM-SHA-256 record.

use super::{super::scram_sha256_verifier::MIN_ITERATIONS, ScramSha256Record, ScramSha256Verifier};

/// Reload сохраняет параметры и проверяемость proof.
#[test]
fn record_round_trip_preserves_verifier_after_reload() {
    let verifier = ScramSha256Verifier::derive(b"pencil", b"RFC salt", MIN_ITERATIONS)
        .expect("valid verifier");
    let record = ScramSha256Record {
        salt: b"RFC salt".to_vec(),
        iterations: MIN_ITERATIONS,
        verifier,
    };
    let encoded = record.encode();
    let loaded = ScramSha256Record::parse(&encoded).expect("valid versioned record");
    assert_eq!(loaded.salt(), record.salt());
    assert_eq!(loaded.iterations(), MIN_ITERATIONS);
    let message = b"client-first,server-first,client-final";
    let proof = client_proof(&loaded, message);
    assert!(loaded.verifier().verify_client_proof(message, &proof));
}

/// Парсер принимает только SHA-256 discriminator, ключи точной длины и cost.
#[test]
fn record_parser_rejects_unknown_versions_and_malformed_fields() {
    let zero_key = "00".repeat(32);
    for version in ["sha1", "unknown"] {
        let encoded =
            format!("litterkeep-scram-{version}-v1$4096$00112233${zero_key}${zero_key}\n");
        assert!(ScramSha256Record::parse(&encoded).is_none());
    }
    let invalid_records = [
        format!("litterkeep-scram-sha256-v2$4096$00112233${zero_key}${zero_key}\n"),
        format!("litterkeep-scram-sha256-v1$200001$00112233${zero_key}${zero_key}\n"),
        format!("litterkeep-scram-sha256-v1$4096$00$00${zero_key}\n"),
        format!("litterkeep-scram-sha256-v1$4096$00112233$00${zero_key}\n"),
        format!("litterkeep-scram-sha256-v1$4096$00112233${zero_key}${zero_key}$extra\n"),
    ];
    for encoded in invalid_records {
        assert!(ScramSha256Record::parse(&encoded).is_none());
    }
}

/// Случайные соли отделяют verifier records одного пароля.
#[test]
fn generated_records_use_distinct_random_salts() {
    let first = ScramSha256Record::create(b"pencil").expect("first credential");
    let second = ScramSha256Record::create(b"pencil").expect("second credential");
    assert_ne!(first.salt(), second.salt());
    assert_ne!(
        first.verifier().stored_key(),
        second.verifier().stored_key()
    );
}

/// Создаёт client proof только для теста формата и crypto round-trip.
fn client_proof(record: &ScramSha256Record, message: &[u8]) -> Vec<u8> {
    let signature =
        super::super::hmac_sha256::authenticate(record.verifier().stored_key(), message);
    let salted_password =
        super::super::pbkdf2_sha256::derive(b"pencil", record.salt(), record.iterations())
            .expect("bounded PBKDF2");
    let mut proof = super::super::hmac_sha256::authenticate(&salted_password, b"Client Key");
    for (proof_octet, signature_octet) in proof.iter_mut().zip(signature) {
        *proof_octet ^= signature_octet;
    }
    proof.to_vec()
}
