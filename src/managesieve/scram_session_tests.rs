//! Интеграционные проверки ManageSieve SCRAM FSM и RFC transcript-а.

use super::{Command, Reply, Session};
use crate::sasl::scram_sha1_record::ScramSha1Record;

/// Проверяет опубликованный server flow от typed command до mutual success.
#[test]
fn scram_exchange_verifies_client_proof_and_returns_server_signature() {
    let (mut session, client_first, credential, server_first) = start_exchange();
    let client_final = client_final(&client_first, &server_first, &credential, false);
    let response = crate::sasl::encode_base64(client_final.as_bytes());
    assert!(matches!(
        session.apply(Command::AuthenticateResponse(response), true, false),
        Reply::ScramAccepted { owner, response }
            if owner == "bob" && contains(&response, b"OK (SASL \"")
    ));
    assert!(matches!(
        session.apply(Command::ListScripts, true, false),
        Reply::List
    ));
}

/// Неверный proof возвращает server-final error и не переводит FSM в logged-in.
#[test]
fn scram_invalid_proof_does_not_authenticate_the_session() {
    let (mut session, client_first, credential, server_first) = start_exchange();
    let client_final = client_final(&client_first, &server_first, &credential, true);
    let response = crate::sasl::encode_base64(client_final.as_bytes());
    assert!(matches!(
        session.apply(Command::AuthenticateResponse(response), true, false),
        Reply::ScramRejected { response }
            if crate::mime::decode_base64(response.as_bytes()).ok().as_deref() == Some(b"e=invalid-proof")
    ));
    assert!(matches!(
        session.apply(Command::ListScripts, true, false),
        Reply::Text(response) if response == b"NO \"invalid command for state\"\r\n"
    ));
}

/// Запускает первый обменный шаг и возвращает transcript с production challenge.
fn start_exchange() -> (
    Session,
    crate::sasl::scram_client_first::ClientFirst,
    ScramSha1Record,
    crate::sasl::scram_server_first::ServerFirst,
) {
    let client_message = "n,,n=bob,r=independent-client-nonce";
    let client_first =
        crate::sasl::scram_client_first::parse(client_message).expect("client-first");
    let encoded = crate::sasl::encode_base64(client_message.as_bytes());
    let mut session = Session::default();
    assert!(matches!(
        session.apply(Command::AuthenticateScramSha1(Some(encoded)), true, false,),
        Reply::ScramClientFirst { .. }
    ));
    assert!(session.begin_scram("bob".into(), client_first.clone()));
    let credential = ScramSha1Record::create(b"pencil").expect("verifier");
    let server_credential = ScramSha1Record::parse(&credential.encode()).expect("server verifier");
    let server_response = match session
        .scram_server_first(server_credential)
        .expect("challenge")
    {
        Reply::ScramChallenge { response } => response,
        _ => panic!("SCRAM server-first challenge was not created"),
    };
    let server_message =
        crate::mime::decode_base64(server_response.as_bytes()).expect("base64 server-first");
    let server_message = String::from_utf8(server_message).expect("UTF-8 server-first");
    let attributes = crate::sasl::scram_attributes::parse(&server_message).expect("attributes");
    let nonce = attributes
        .iter()
        .find(|attribute| attribute.name == crate::sasl::scram_attributes::AttributeName::Nonce)
        .map(|attribute| attribute.value.to_owned())
        .expect("server nonce");
    (
        session,
        client_first,
        credential,
        crate::sasl::scram_server_first::ServerFirst {
            nonce: nonce.to_owned(),
            message: server_message,
        },
    )
}

/// Строит client-final independently из password и transcript-а.
fn client_final(
    client: &crate::sasl::scram_client_first::ClientFirst,
    server: &crate::sasl::scram_server_first::ServerFirst,
    credential: &ScramSha1Record,
    invalid: bool,
) -> String {
    let unsigned = format!("c=biws,r={}", server.nonce);
    let auth_message = format!(
        "{},{},{}",
        client.client_first_bare, server.message, unsigned
    );
    let salted =
        crate::sasl::pbkdf2_sha1::derive(b"pencil", credential.salt(), credential.iterations())
            .expect("salted password");
    let client_key = crate::sasl::hmac_sha1::authenticate(&salted, b"Client Key");
    let stored_key = crate::sasl::sha1::digest(&client_key);
    let signature = crate::sasl::hmac_sha1::authenticate(&stored_key, auth_message.as_bytes());
    let proof = match invalid {
        true => vec![0; client_key.len()],
        false => client_key
            .iter()
            .zip(signature)
            .map(|(key, signature)| *key ^ signature)
            .collect(),
    };
    format!("{unsigned},p={}", crate::sasl::encode_base64(&proof))
}

/// Проверяет наличие wire suffix без индексации бинарного response.
fn contains(response: &[u8], needle: &[u8]) -> bool {
    response
        .windows(needle.len())
        .any(|window| window == needle)
}
