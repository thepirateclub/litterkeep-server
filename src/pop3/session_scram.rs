//! SCRAM-SHA-1 exchange в POP3 AUTHORIZATION state.

use super::{Phase, PriorAuthorization, Reply, SaslInitialResponse, Session};
use crate::sasl::{
    scram_client_final,
    scram_client_first::{self, ClientFirst},
    scram_server_final::ServerFinal,
    scram_server_first::ServerFirst,
    scram_sha1_record::ScramSha1Record,
};

/// Временный transcript, который живёт только до конца одного POP3 AUTH.
pub(super) struct Exchange {
    account: String,
    client: ClientFirst,
    server: Option<ServerFirst>,
    verifier: Option<ScramSha1Record>,
    proof_verified: bool,
}

/// Типизирует cancellation marker отдельно от непрозрачного SCRAM payload.
pub(super) enum ClientResponse<'a> {
    Cancel,
    Payload(&'a str),
}

impl<'a> ClientResponse<'a> {
    /// Различает единственный RFC 5034 marker отмены перед парсером SASL data.
    pub(super) fn parse(value: &'a str) -> Self {
        match value.as_bytes() {
            b"*" => Self::Cancel,
            _ => Self::Payload(value),
        }
    }
}

/// Запускает механизм с initial response или запрашивает первый challenge.
pub(super) fn start(session: &mut Session, response: Option<SaslInitialResponse>) -> Reply {
    if !session.tls_active {
        return Reply::Error("STLS required before AUTH SCRAM-SHA-1");
    }
    session.sasl_previous_authorization = Some(match &session.username {
        Some(username) => PriorAuthorization::User(username.clone()),
        None => PriorAuthorization::NoUser,
    });
    match response {
        None => {
            session.phase = Phase::SaslScramFirst;
            Reply::SaslContinue
        }
        Some(SaslInitialResponse::Encoded(value)) => client_first(session, &value),
        Some(SaslInitialResponse::Empty) => reject(session, "invalid SCRAM client-first message"),
    }
}

/// Разбирает canonical Base64 client-first и задаёт внутренний account key.
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
    session.scram_exchange = Some(Exchange {
        account: account.clone(),
        client,
        server: None,
        verifier: None,
        proof_verified: false,
    });
    session.phase = Phase::SaslScramVerifier;
    Reply::ScramVerifierLookup { account }
}

/// Принимает verifier worker-а и отправляет RFC 5802 server-first challenge.
pub(super) fn verifier_loaded(
    session: &mut Session,
    result: std::io::Result<Option<ScramSha1Record>>,
) -> Reply {
    let credential = match result {
        Ok(Some(credential)) => credential,
        Ok(None) => return reject(session, "authentication failed"),
        Err(_) => return reject(session, "authentication temporarily unavailable"),
    };
    let Some(exchange) = session.scram_exchange.as_mut() else {
        return reject(session, "authentication failed");
    };
    let server = match ServerFirst::create(&exchange.client, &credential) {
        Ok(server) => server,
        Err(_) => return reject(session, "authentication temporarily unavailable"),
    };
    let challenge = crate::sasl::encode_base64(server.message.as_bytes());
    exchange.server = Some(server);
    exchange.verifier = Some(credential);
    session.phase = Phase::SaslScramFinal;
    Reply::SaslChallenge(challenge)
}

/// Проверяет client-final proof и выдаёт server-final отдельным challenge.
pub(super) fn client_final(session: &mut Session, encoded: &str) -> Reply {
    let Some(message) = decode(encoded) else {
        return reject(session, "authentication failed");
    };
    let Some(exchange) = session.scram_exchange.as_mut() else {
        return reject(session, "authentication failed");
    };
    let Some(server) = exchange.server.as_ref() else {
        return reject(session, "authentication failed");
    };
    let Some(credential) = exchange.verifier.as_ref() else {
        return reject(session, "authentication failed");
    };
    let final_message = match scram_client_final::parse(&message, &server.nonce) {
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
    session.phase = Phase::SaslScramAcknowledgement;
    Reply::SaslChallenge(response)
}

/// Завершает exchange пустым client response, как в SASL DIGEST/SCRAM flow.
pub(super) fn acknowledgement(session: &mut Session, response: &str) -> Reply {
    match response.is_empty() {
        false => reject(session, "authentication failed"),
        true => match session.scram_exchange.take() {
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

/// Отклоняет malformed/unknown exchange одинаковым ответом и восстанавливает USER.
fn reject(session: &mut Session, reason: &'static str) -> Reply {
    session.restore_authorization_after_sasl();
    Reply::Error(reason)
}

/// Декодирует только canonical Base64 UTF-8 mechanism data.
fn decode(encoded: &str) -> Option<String> {
    let decoded = crate::mime::decode_base64(encoded.as_bytes()).ok()?;
    match crate::sasl::encode_base64(&decoded) == encoded {
        true => String::from_utf8(decoded).ok(),
        false => None,
    }
}

#[cfg(test)]
#[path = "session_scram_tests.rs"]
mod tests;
