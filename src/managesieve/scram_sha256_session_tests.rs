//! Проверяет SHA-256 ManageSieve exchange на transcript и server signature.

use super::{Command, Reply, Session};
use crate::sasl::scram_sha256_record::ScramSha256Record;

/// Клиент независимо вычисляет proof и сверяет взаимную server signature.
#[test]
fn scram_sha256_exchange_verifies_proof_and_server_signature() {
    let (mut session, client, record, server) = start();
    let final_message = client_final(&client, &server, &record, false);
    let response = crate::sasl::encode_base64(final_message.as_bytes());
    let (owner, response) =
        match session.apply(Command::AuthenticateResponse(response), true, false) {
            Reply::ScramSha256Accepted { owner, response } => (owner, response),
            _ => panic!("valid SHA-256 proof was rejected"),
        };
    assert_eq!(owner, "bob");
    assert!(response.windows(10).any(|part| part == b"OK (SASL \""));
}

/// Ошибочный proof не выдаёт authenticated state и сохраняет отказ RFC.
#[test]
fn scram_sha256_invalid_proof_does_not_authenticate() {
    let (mut session, client, record, server) = start();
    let final_message = client_final(&client, &server, &record, true);
    let response = crate::sasl::encode_base64(final_message.as_bytes());
    assert!(matches!(
        session.apply(Command::AuthenticateResponse(response), true, false),
        Reply::ScramSha256Rejected { response }
            if crate::mime::decode_base64(response.as_bytes()).ok().as_deref() == Some(b"e=invalid-proof")
    ));
    assert!(matches!(
        session.apply(Command::ListScripts, true, false),
        Reply::Text(response) if response == b"NO \"invalid command for state\"\r\n"
    ));
}

fn start() -> (
    Session,
    crate::sasl::scram_client_first::ClientFirst,
    ScramSha256Record,
    crate::sasl::scram_sha256_server_first::ServerFirst,
) {
    let message = "n,,n=bob,r=sha256-independent-nonce";
    let client = crate::sasl::scram_client_first::parse(message).expect("client-first");
    let encoded = crate::sasl::encode_base64(message.as_bytes());
    let mut session = Session::default();
    assert!(matches!(
        session.apply(Command::AuthenticateScramSha256(Some(encoded)), true, false),
        Reply::ScramSha256ClientFirst { .. }
    ));
    assert!(session.begin_scram_sha256("bob".into(), client.clone()));
    let record = ScramSha256Record::create(b"pencil").expect("verifier record");
    let server_response = match session
        .scram_sha256_server_first(ScramSha256Record::parse(&record.encode()).expect("reload"))
        .expect("server-first")
    {
        Reply::ScramSha256Challenge { response } => response,
        _ => panic!("SHA-256 challenge expected"),
    };
    let decoded = crate::mime::decode_base64(server_response.as_bytes()).expect("base64");
    let message = String::from_utf8(decoded).expect("UTF-8");
    let attributes = crate::sasl::scram_attributes::parse(&message).expect("attributes");
    let nonce = attributes
        .iter()
        .find(|attribute| attribute.name == crate::sasl::scram_attributes::AttributeName::Nonce)
        .map(|attribute| attribute.value.clone())
        .expect("server nonce");
    (
        session,
        client,
        record,
        crate::sasl::scram_sha256_server_first::ServerFirst { nonce, message },
    )
}

fn client_final(
    client: &crate::sasl::scram_client_first::ClientFirst,
    server: &crate::sasl::scram_sha256_server_first::ServerFirst,
    record: &ScramSha256Record,
    invalid: bool,
) -> String {
    let unsigned = format!("c=biws,r={}", server.nonce);
    let auth_message = format!(
        "{},{},{}",
        client.client_first_bare, server.message, unsigned
    );
    let salted = crate::sasl::pbkdf2_sha256::derive(b"pencil", record.salt(), record.iterations())
        .expect("salted password");
    let client_key = crate::sasl::hmac_sha256::authenticate(&salted, b"Client Key");
    let stored_key = crate::sasl::sha256::digest(&client_key);
    let signature = crate::sasl::hmac_sha256::authenticate(&stored_key, auth_message.as_bytes());
    let proof = match invalid {
        true => vec![0; 32],
        false => client_key
            .iter()
            .zip(signature)
            .map(|(key, signature)| *key ^ signature)
            .collect(),
    };
    format!("{unsigned},p={}", crate::sasl::encode_base64(&proof))
}
