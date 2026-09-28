//! Отдельные переходы ManageSieve state machine для SCRAM-SHA-256.

use super::{Phase, Reply, ScramExchange, Session, capability_lines, no};

impl Session {
    /// Связывает SHA-256 client-first с local account до owner lookup.
    pub fn begin_scram_sha256(
        &mut self,
        owner: String,
        client_first: crate::sasl::scram_client_first::ClientFirst,
    ) -> bool {
        match self.phase {
            Phase::AwaitingScramSha256Verifier => {
                self.scram_exchange = Some(ScramExchange {
                    owner,
                    client_first,
                    server_first: None,
                    credential: None,
                    sha256_server_first: None,
                    sha256_credential: None,
                });
                true
            }
            _ => false,
        }
    }

    /// Сохраняет SHA-256 transcript и переводит сессию к client-final.
    pub fn scram_sha256_server_first(
        &mut self,
        credential: crate::sasl::scram_sha256_record::ScramSha256Record,
    ) -> std::io::Result<Reply> {
        let Some(exchange) = self.scram_exchange.as_mut() else {
            return Err(std::io::Error::other("SCRAM exchange is missing"));
        };
        let server_first =
            crate::sasl::scram_sha256_server_first::create(&exchange.client_first, &credential)?;
        let response = crate::sasl::encode_base64(server_first.message.as_bytes());
        exchange.sha256_server_first = Some(server_first);
        exchange.sha256_credential = Some(credential);
        self.phase = Phase::AwaitingScramSha256Final;
        Ok(Reply::ScramSha256Challenge { response })
    }
}

/// Обрабатывает cancellation или initial response после пустого SASL challenge.
pub(super) fn first_response(session: &mut Session, response: String) -> Reply {
    match crate::managesieve::SaslResponse::parse(response) {
        crate::managesieve::SaslResponse::Cancel => {
            session.scram_failed();
            no("authentication cancelled")
        }
        crate::managesieve::SaslResponse::Payload(response) => {
            session.phase = Phase::AwaitingScramSha256Verifier;
            Reply::ScramSha256ClientFirst { response }
        }
    }
}

/// Обрабатывает client-final или отмену SHA-256 exchange.
pub(super) fn final_response(session: &mut Session, response: String) -> Reply {
    match crate::managesieve::SaslResponse::parse(response) {
        crate::managesieve::SaslResponse::Cancel => {
            session.scram_failed();
            no("authentication cancelled")
        }
        crate::managesieve::SaslResponse::Payload(response) => finish(session, &response),
    }
}

fn finish(session: &mut Session, response: &str) -> Reply {
    let Some(exchange) = session.scram_exchange.take() else {
        session.phase = Phase::Plain;
        return no("authentication failed");
    };
    let outcome = super::super::scram_sha256_final::verify(&exchange, response);
    session.phase = Phase::Plain;
    match outcome {
        super::super::scram_sha256_final::Outcome::Verified(server_final) => {
            accept(session, exchange, server_final)
        }
        super::super::scram_sha256_final::Outcome::Rejected(server_final) => reject(server_final),
    }
}

fn accept(session: &mut Session, exchange: ScramExchange, server_final: String) -> Reply {
    session.phase = Phase::Authenticated {
        owner: exchange.owner.clone(),
    };
    let final_data = crate::sasl::encode_base64(server_final.as_bytes());
    let ending = format!("OK (SASL \"{final_data}\") \"authenticated\"\r\n");
    let response = capability_lines(true, false, Some(&exchange.owner), ending.as_bytes());
    Reply::ScramSha256Accepted {
        owner: exchange.owner,
        response,
    }
}

fn reject(server_final: String) -> Reply {
    Reply::ScramSha256Rejected {
        response: crate::sasl::encode_base64(server_final.as_bytes()),
    }
}
