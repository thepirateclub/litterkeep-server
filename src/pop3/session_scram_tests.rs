//! Проверки переходов POP3 SCRAM и fail-closed обработки proof.

use super::*;

#[test]
fn accepts_only_tls_protected_scram_capability() {
    let mut session = Session::default();
    let plaintext = session.apply(crate::pop3::parse("AUTH SCRAM-SHA-1\r\n"));
    assert!(matches!(plaintext, Reply::Error(_)));
    session.tls_established();
    let encrypted = session.apply(crate::pop3::parse("CAPA\r\n"));
    assert!(matches!(encrypted, Reply::Multiline(bytes)
        if bytes.windows(b"SCRAM-SHA-1 SCRAM-SHA-256\r\n".len())
            .any(|line| line == b"SCRAM-SHA-1 SCRAM-SHA-256\r\n")));
}

#[test]
fn rejects_bad_client_proof_after_server_final_challenge() {
    let mut session = Session::default();
    session.tls_established();
    let first = crate::sasl::encode_base64(b"n,,n=bob,r=client-nonce");
    let command = crate::pop3::parse(&format!("AUTH SCRAM-SHA-1 {first}\r\n"));
    assert!(matches!(
        session.apply(command),
        Reply::ScramVerifierLookup { account } if account == "bob"
    ));

    let credential = ScramSha1Record::create(b"secret-password").expect("test verifier");
    let Reply::SaslChallenge(server_first) = verifier_loaded(&mut session, Ok(Some(credential)))
    else {
        panic!("verifier must produce a server-first challenge");
    };
    let decoded = decode(&server_first).expect("server-first Base64");
    let attributes = crate::sasl::scram_attributes::parse(&decoded).expect("server attributes");
    let nonce = attributes
        .iter()
        .find(|attribute| attribute.name == crate::sasl::scram_attributes::AttributeName::Nonce)
        .map(|attribute| attribute.value.as_str())
        .expect("server nonce");
    let invalid = crate::sasl::encode_base64(
        format!("c=biws,r={nonce},p=AAAAAAAAAAAAAAAAAAAAAAAAAAA=").as_bytes(),
    );
    assert!(matches!(
        client_final(&mut session, &invalid),
        Reply::SaslChallenge(_)
    ));
    assert!(matches!(
        acknowledgement(&mut session, ""),
        Reply::Error("authentication failed")
    ));
    assert!(!session.awaiting_sasl_response());
}

#[test]
fn cancellation_restores_the_prior_user_and_allows_retry() {
    let mut session = Session::default();
    session.tls_established();
    assert!(matches!(
        session.apply(crate::pop3::parse("USER bob\r\n")),
        Reply::Ok(_)
    ));
    assert!(matches!(start(&mut session, None), Reply::SaslContinue));
    assert!(matches!(
        session.sasl_plain_response("*"),
        Reply::Error("authentication cancelled")
    ));
    assert!(matches!(
        session.apply(crate::pop3::parse("PASS allowed-after-cancel\r\n")),
        Reply::Authenticate { username, .. } if username == "bob"
    ));
}
