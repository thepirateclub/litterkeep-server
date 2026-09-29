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

/// PLUS сверяет GS2 header вместе с exporter и проверяет взаимную подпись.
#[test]
fn scram_sha256_plus_binds_exact_tls_exporter_and_rejects_mismatch() {
    let exporter = b"independent tls exporter bytes";
    let message = "p=tls-exporter,,n=bob,r=plus-client-nonce";
    let client =
        crate::sasl::scram_client_first::parse_tls_exporter(message).expect("PLUS client-first");
    let encoded = crate::sasl::encode_base64(message.as_bytes());
    let mut session = Session::default();
    assert!(matches!(
        session.apply_with_channel_binding(
            Command::AuthenticateScramSha256Plus(Some(encoded)),
            true,
            false,
            Some(exporter)
        ),
        Reply::ScramSha256ClientFirst {
            channel_binding: Some(_),
            ..
        }
    ));
    assert!(session.begin_scram_sha256("bob".into(), client.clone()));
    let record = ScramSha256Record::create(b"pencil").expect("verifier");
    let challenge = match session
        .scram_sha256_server_first(reload_record(&record))
        .expect("challenge")
    {
        Reply::ScramSha256Challenge { response } => response,
        _ => panic!("PLUS challenge expected"),
    };
    let server = parse_server_first(&challenge);
    let final_message = client_final_with_binding(&client, &server, &record, false, exporter);
    let response = crate::sasl::encode_base64(final_message.as_bytes());
    assert!(matches!(
        session.apply_with_channel_binding(Command::AuthenticateResponse(response), true, false, Some(exporter)),
        Reply::ScramSha256Accepted { owner, .. } if owner == "bob"
    ));

    let mut rejected = Session::default();
    let encoded = crate::sasl::encode_base64(message.as_bytes());
    let _ = rejected.apply_with_channel_binding(
        Command::AuthenticateScramSha256Plus(Some(encoded)),
        true,
        false,
        Some(exporter),
    );
    assert!(rejected.begin_scram_sha256("bob".into(), client.clone()));
    let second_challenge = match rejected
        .scram_sha256_server_first(reload_record(&record))
        .expect("challenge")
    {
        Reply::ScramSha256Challenge { response } => response,
        _ => panic!("second PLUS challenge expected"),
    };
    let second_server = parse_server_first(&second_challenge);
    let wrong =
        client_final_with_binding(&client, &second_server, &record, false, b"wrong exporter");
    let response = crate::sasl::encode_base64(wrong.as_bytes());
    assert!(matches!(
        rejected.apply_with_channel_binding(
            Command::AuthenticateResponse(response),
            true,
            false,
            Some(exporter)
        ),
        Reply::ScramSha256Rejected { .. }
    ));
}

fn parse_server_first(encoded: &str) -> crate::sasl::scram_sha256_server_first::ServerFirst {
    let decoded = crate::mime::decode_base64(encoded.as_bytes()).expect("base64");
    let message = String::from_utf8(decoded).expect("UTF-8");
    let attributes = crate::sasl::scram_attributes::parse(&message).expect("fields");
    let nonce = attributes
        .iter()
        .find(|field| field.name == crate::sasl::scram_attributes::AttributeName::Nonce)
        .map(|field| field.value.to_owned())
        .expect("nonce");
    crate::sasl::scram_sha256_server_first::ServerFirst {
        nonce: nonce.to_owned(),
        message,
    }
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
        .map(|attribute| attribute.value.to_owned())
        .expect("server nonce");
    (
        session,
        client,
        record,
        crate::sasl::scram_sha256_server_first::ServerFirst {
            nonce: nonce.to_owned(),
            message,
        },
    )
}

fn client_final(
    client: &crate::sasl::scram_client_first::ClientFirst,
    server: &crate::sasl::scram_sha256_server_first::ServerFirst,
    record: &ScramSha256Record,
    invalid: bool,
) -> String {
    client_final_with_binding(client, server, record, invalid, b"")
}

fn client_final_with_binding(
    client: &crate::sasl::scram_client_first::ClientFirst,
    server: &crate::sasl::scram_sha256_server_first::ServerFirst,
    record: &ScramSha256Record,
    invalid: bool,
    exporter: &[u8],
) -> String {
    let mut binding = client.gs2_header.as_bytes().to_vec();
    binding.extend_from_slice(exporter);
    let encoded_binding = crate::sasl::encode_base64(&binding);
    let unsigned = format!("c={encoded_binding},r={}", server.nonce);
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

fn reload_record(record: &ScramSha256Record) -> ScramSha256Record {
    ScramSha256Record::parse(&record.encode()).expect("parse verifier copy")
}
