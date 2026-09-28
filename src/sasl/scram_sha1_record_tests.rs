//! Проверки устойчивости и отказа закрытого SCRAM verifier record format.
use super::*;

#[test]
fn generated_record_round_trips_and_proof_verification_survives_reload() {
    let verifier = ScramSha1Verifier::derive(b"pencil", b"RFC salt", MIN_ITERATIONS)
        .expect("valid RFC verifier");
    let record = ScramSha1Record {
        salt: b"RFC salt".to_vec(),
        iterations: MIN_ITERATIONS,
        verifier,
    };
    let encoded = record.encode();
    let loaded = ScramSha1Record::parse(&encoded).expect("valid versioned record");

    assert_eq!(loaded.salt(), record.salt());
    assert_eq!(loaded.iterations(), MIN_ITERATIONS);
    let message = b"client-first,server-first,client-final";
    let proof = proof_for(&loaded, message);
    assert!(loaded.verifier().verify_client_proof(message, &proof));
}

/// Вычисляет корректный тестовый proof из ClientKey для записи.
fn proof_for(record: &ScramSha1Record, message: &[u8]) -> Vec<u8> {
    let signature = super::super::hmac_sha1::authenticate(record.verifier.stored_key(), message);
    let mut proof = super::super::hmac_sha1::authenticate(
        &super::super::pbkdf2_sha1::derive(b"pencil", record.salt(), record.iterations())
            .expect("salted password"),
        b"Client Key",
    );
    for (proof_octet, signature_octet) in proof.iter_mut().zip(signature) {
        *proof_octet ^= signature_octet;
    }
    proof.to_vec()
}

#[test]
fn parser_rejects_unknown_versions_bad_lengths_and_unbounded_iterations() {
    for record in [
        "litterkeep-scram-sha1-v2$4096$00112233$0000000000000000000000000000000000000000$0000000000000000000000000000000000000000\n",
        "litterkeep-scram-sha1-v1$4096$00$00$0000000000000000000000000000000000000000\n",
        "litterkeep-scram-sha1-v1$200001$00112233$0000000000000000000000000000000000000000$0000000000000000000000000000000000000000\n",
        "litterkeep-scram-sha1-v1$4096$00112233$0000000000000000000000000000000000000000$0000000000000000000000000000000000000000$extra\n",
    ] {
        assert!(ScramSha1Record::parse(record).is_none());
    }
}

#[test]
fn generated_records_use_distinct_cryptographic_salts() {
    let first = ScramSha1Record::create(b"pencil").expect("first verifier");
    let second = ScramSha1Record::create(b"pencil").expect("second verifier");
    assert_ne!(first.salt(), second.salt());
    assert_ne!(
        first.verifier().stored_key(),
        second.verifier().stored_key()
    );
}
