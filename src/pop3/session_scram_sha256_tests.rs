//! POP3 SCRAM-SHA-256 FSM и взаимная проверка RFC 7677 proof.

use super::*;

#[test]
fn sha256_capability_requires_tls_and_success_reaches_maildrop() {
    let mut session = Session::default();
    assert!(matches!(
        session.apply(crate::pop3::parse("AUTH SCRAM-SHA-256\r\n")),
        Reply::Error(_)
    ));
    session.tls_established();
    let capability = session.apply(crate::pop3::parse("CAPA\r\n"));
    assert!(matches!(capability, Reply::Multiline(bytes)
        if bytes.windows(b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256\r\n".len())
            .any(|line| line == b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256\r\n")));

    let first = crate::sasl::encode_base64(b"n,,n=bob,r=client-nonce");
    let lookup = session.apply(crate::pop3::parse(&format!(
        "AUTH SCRAM-SHA-256 {first}\r\n"
    )));
    assert!(matches!(lookup, Reply::ScramSha256VerifierLookup { account } if account == "bob"));

    let record = crate::sasl::scram_sha256_record::ScramSha256Record::create(b"pencil")
        .expect("test verifier");
    let salt = record.salt().to_vec();
    let iterations = record.iterations();
    let challenge = session.scram_sha256_verifier_loaded(Ok(Some(record)));
    let Reply::SaslChallenge(server_first_encoded) = challenge else {
        panic!("verifier lookup must produce SCRAM server-first");
    };
    let server_first = decode(&server_first_encoded).expect("server-first Base64");
    let nonce = field(&server_first, "r");
    let client_bare = "n=bob,r=client-nonce";
    let client_without_proof = format!("c=biws,r={nonce}");
    let auth_message = format!("{client_bare},{server_first},{client_without_proof}");
    let (proof, server_signature) =
        proof_and_signature(b"pencil", &salt, iterations, &auth_message);
    let final_message = format!(
        "{client_without_proof},p={}",
        crate::sasl::encode_base64(&proof)
    );
    let final_response =
        session.sasl_plain_response(&crate::sasl::encode_base64(final_message.as_bytes()));
    let Reply::SaslChallenge(server_final) = final_response else {
        panic!("valid client proof must receive server-final");
    };
    assert_eq!(
        decode(&server_final).as_deref(),
        Some(format!("v={}", crate::sasl::encode_base64(&server_signature)).as_str())
    );
    assert!(
        matches!(session.sasl_plain_response(""), Reply::ScramAccepted { account } if account == "bob")
    );
    assert!(matches!(session.authenticated(Vec::new()), Reply::Ok(_)));
    let post_auth_capability = session.apply(crate::pop3::parse("CAPA\r\n"));
    assert!(matches!(post_auth_capability, Reply::Multiline(bytes)
        if bytes.windows(b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256\r\n".len())
            .any(|line| line == b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256\r\n")));
    assert!(matches!(
        session.apply(crate::pop3::parse("AUTH PLAIN\r\n")),
        Reply::Error("invalid command for state")
    ));
}

#[test]
fn sha256_invalid_proof_never_authenticates_or_retains_exchange() {
    let mut session = Session::default();
    session.tls_established();
    let first = crate::sasl::encode_base64(b"n,,n=bob,r=client-nonce");
    assert!(matches!(
        session.apply(crate::pop3::parse(&format!(
            "AUTH SCRAM-SHA-256 {first}\r\n"
        ))),
        Reply::ScramSha256VerifierLookup { .. }
    ));
    let record = crate::sasl::scram_sha256_record::ScramSha256Record::create(b"pencil")
        .expect("test verifier");
    let Reply::SaslChallenge(server_first) = session.scram_sha256_verifier_loaded(Ok(Some(record)))
    else {
        panic!("server-first challenge expected");
    };
    let server_first = decode(&server_first).expect("server-first Base64");
    let nonce = field(&server_first, "r");
    let invalid = crate::sasl::encode_base64(
        format!("c=biws,r={nonce},p=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").as_bytes(),
    );
    assert!(matches!(
        session.sasl_plain_response(&invalid),
        Reply::SaslChallenge(_)
    ));
    assert!(matches!(
        session.sasl_plain_response(""),
        Reply::Error("authentication failed")
    ));
    assert!(matches!(
        session.authenticated(Vec::new()),
        Reply::Error("USER required")
    ));
}

#[test]
fn sha256_plus_requires_tls_exporter_and_verifies_channel_binding() {
    let mut session = Session::default();
    session.tls_established();
    assert!(matches!(
        session.apply(crate::pop3::parse("AUTH SCRAM-SHA-256-PLUS\r\n")),
        Reply::Error(_)
    ));
    session.update_tls_exporter(Some([0x5a; 32]));
    let capability = session.apply(crate::pop3::parse("CAPA\r\n"));
    assert!(matches!(capability, Reply::Multiline(bytes)
        if bytes.windows(b"SCRAM-SHA-256-PLUS".len())
            .any(|part| part == b"SCRAM-SHA-256-PLUS")));

    let client_first = "p=tls-exporter,,n=bob,r=plus-client-nonce";
    let encoded = crate::sasl::encode_base64(client_first.as_bytes());
    assert!(matches!(
        session.apply(crate::pop3::parse(&format!(
            "AUTH SCRAM-SHA-256-PLUS {encoded}\r\n"
        ))),
        Reply::ScramSha256VerifierLookup { .. }
    ));
    let record = crate::sasl::scram_sha256_record::ScramSha256Record::create(b"pencil")
        .expect("test verifier");
    let salt = record.salt().to_vec();
    let iterations = record.iterations();
    let Reply::SaslChallenge(server_first_encoded) =
        session.scram_sha256_verifier_loaded(Ok(Some(record)))
    else {
        panic!("PLUS verifier lookup must produce server-first");
    };
    let server_first = decode(&server_first_encoded).expect("server-first Base64");
    let nonce = field(&server_first, "r");
    let client_bare = "n=bob,r=plus-client-nonce";
    let mut gs2_binding = b"p=tls-exporter,,".to_vec();
    gs2_binding.extend_from_slice(&[0x5a; 32]);
    let client_without_proof = format!("c={},r={nonce}", crate::sasl::encode_base64(&gs2_binding));
    let auth_message = format!("{client_bare},{server_first},{client_without_proof}");
    let (proof, server_signature) =
        proof_and_signature(b"pencil", &salt, iterations, &auth_message);
    let final_message = format!(
        "{client_without_proof},p={}",
        crate::sasl::encode_base64(&proof)
    );
    let final_response =
        session.sasl_plain_response(&crate::sasl::encode_base64(final_message.as_bytes()));
    let Reply::SaslChallenge(server_final) = final_response else {
        panic!("valid PLUS proof must receive server-final");
    };
    assert_eq!(
        decode(&server_final).as_deref(),
        Some(format!("v={}", crate::sasl::encode_base64(&server_signature)).as_str())
    );
    assert!(
        matches!(session.sasl_plain_response(""), Reply::ScramAccepted { account } if account == "bob")
    );
}

fn field<'a>(message: &'a str, key: &str) -> &'a str {
    message
        .split(',')
        .find_map(|field| field.strip_prefix(&format!("{key}=")))
        .expect("required SCRAM field")
}

fn decode(encoded: &str) -> Option<String> {
    String::from_utf8(crate::mime::decode_base64(encoded.as_bytes()).ok()?).ok()
}

fn proof_and_signature(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    auth_message: &str,
) -> (Vec<u8>, [u8; 32]) {
    let salted =
        crate::sasl::pbkdf2_sha256::derive(password, salt, iterations).expect("salted password");
    let client_key = crate::sasl::hmac_sha256::authenticate(&salted, b"Client Key");
    let stored_key = crate::sasl::sha256::digest(&client_key);
    let client_signature =
        crate::sasl::hmac_sha256::authenticate(&stored_key, auth_message.as_bytes());
    let proof = client_key
        .iter()
        .zip(client_signature)
        .map(|(key, signature)| *key ^ signature)
        .collect();
    let server_key = crate::sasl::hmac_sha256::authenticate(&salted, b"Server Key");
    let signature = crate::sasl::hmac_sha256::authenticate(&server_key, auth_message.as_bytes());
    (proof, signature)
}
