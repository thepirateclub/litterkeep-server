//! SCRAM-SHA-256 exchange в POP3 AUTHORIZATION state.

use super::{Phase, PriorAuthorization, Reply, SaslInitialResponse, Session};
use crate::sasl::{
    scram_client_first::{self, ClientFirst},
    scram_sha256_client_final,
    scram_sha256_record::ScramSha256Record,
    scram_sha256_server_final::ServerFinal,
    scram_sha256_server_first::ServerFirst,
};

/// Transcript одного POP3 AUTH; verifier живёт только до завершения exchange.
pub(super) struct Exchange {
    account: String,
    client: ClientFirst,
    server: Option<ServerFirst>,
    verifier: Option<ScramSha256Record>,
    proof_verified: bool,
}

/// Начинает SASL exchange только внутри TLS.
pub(super) fn start(session: &mut Session, response: Option<SaslInitialResponse>) -> Reply {
    if !session.tls_active {
        return Reply::Error("STLS required before AUTH SCRAM-SHA-256");
    }
    session.sasl_previous_authorization = Some(match &session.username {
        Some(username) => PriorAuthorization::User(username.clone()),
        None => PriorAuthorization::NoUser,
    });
    match response {
        None => {
            session.phase = Phase::SaslScramSha256First;
            Reply::SaslContinue
        }
        Some(SaslInitialResponse::Encoded(value)) => client_first(session, &value),
        Some(SaslInitialResponse::Empty) => reject(session, "invalid SCRAM client-first message"),
    }
}

/// Проверяет Base64 client-first и запрашивает verifier через Accounts owner.
pub(super) fn client_first(session: &mut Session, encoded: &str) -> Reply {
    let Some(message) = decode(encoded) else {
        return reject(session, "authentication failed");
    };
    let client = match scram_client_first::parse(&message) {
        Ok(client) => client,
        Err(_) => return reject(session, "authentication failed"),
    };
    let Some(account) = crate::local_route::local_login(&client.username) else {
        return reject(session, "authentication failed");
    };
    session.scram_sha256_exchange = Some(Exchange {
        account: account.clone(),
        client,
        server: None,
        verifier: None,
        proof_verified: false,
    });
    session.phase = Phase::SaslScramSha256Verifier;
    Reply::ScramSha256VerifierLookup { account }
}

/// Создаёт server-first challenge после ответа bounded StorageWorker.
pub(super) fn verifier_loaded(
    session: &mut Session,
    result: std::io::Result<Option<ScramSha256Record>>,
) -> Reply {
    let credential = match result {
        Ok(Some(credential)) => credential,
        Ok(None) => return reject(session, "authentication failed"),
        Err(_) => return reject(session, "authentication temporarily unavailable"),
    };
    let Some(exchange) = session.scram_sha256_exchange.as_mut() else {
        return reject(session, "authentication failed");
    };
    let server = match crate::sasl::scram_sha256_server_first::create(&exchange.client, &credential)
    {
        Ok(server) => server,
        Err(_) => return reject(session, "authentication temporarily unavailable"),
    };
    let challenge = crate::sasl::encode_base64(server.message.as_bytes());
    exchange.server = Some(server);
    exchange.verifier = Some(credential);
    session.phase = Phase::SaslScramSha256Final;
    Reply::SaslChallenge(challenge)
}

/// Проверяет client proof и отправляет server-final отдельным continuation.
pub(super) fn client_final(session: &mut Session, encoded: &str) -> Reply {
    let Some(message) = decode(encoded) else {
        return reject(session, "authentication failed");
    };
    let Some(exchange) = session.scram_sha256_exchange.as_mut() else {
        return reject(session, "authentication failed");
    };
    let (Some(server), Some(credential)) = (exchange.server.as_ref(), exchange.verifier.as_ref())
    else {
        return reject(session, "authentication failed");
    };
    let final_message = match scram_sha256_client_final::parse(&message, &server.nonce) {
        Ok(final_message) => final_message,
        Err(_) => return reject(session, "authentication failed"),
    };
    let server_final = ServerFinal::complete(
        &exchange.client,
        server,
        &final_message,
        credential.verifier(),
    );
    exchange.proof_verified = matches!(server_final, ServerFinal::Verified(_));
    let response = crate::sasl::encode_base64(server_final.encode().as_bytes());
    session.phase = Phase::SaslScramSha256Acknowledgement;
    Reply::SaslChallenge(response)
}

/// Завершает SCRAM пустым client response и допускает выдачу maildrop.
pub(super) fn acknowledgement(session: &mut Session, response: &str) -> Reply {
    match response.is_empty() {
        false => reject(session, "authentication failed"),
        true => match session.scram_sha256_exchange.take() {
            Some(exchange) if exchange.proof_verified => {
                session.username = Some(exchange.account.clone());
                session.phase = Phase::UserAccepted;
                session.sasl_previous_authorization = None;
                Reply::ScramAccepted {
                    account: exchange.account,
                }
            }
            Some(_) | None => reject(session, "authentication failed"),
        },
    }
}

/// Сбрасывает transcript и восстанавливает прежнюю пару USER/PASS.
fn reject(session: &mut Session, reason: &'static str) -> Reply {
    session.restore_authorization_after_sasl();
    Reply::Error(reason)
}

/// Принимает только canonical Base64 с UTF-8 SCRAM сообщением.
fn decode(encoded: &str) -> Option<String> {
    let decoded = crate::mime::decode_base64(encoded.as_bytes()).ok()?;
    match crate::sasl::encode_base64(&decoded) == encoded {
        true => String::from_utf8(decoded).ok(),
        false => None,
    }
}

#[cfg(test)]
#[path = "session_scram_sha256_tests.rs"]
mod tests;
