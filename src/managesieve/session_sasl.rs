//! Состояния ManageSieve команд и SASL challenge-response.

use super::super::{Command, SaslResponse};
use super::{Phase, Reply, Session, capability, capability_lines, no, noop, ok};

impl Session {
    /// Применяет одну команду к текущему authenticated/SASL состоянию.
    pub fn apply(&mut self, command: Command, tls_active: bool, starttls_available: bool) -> Reply {
        match (&self.phase, command) {
            (Phase::Plain, Command::Capability) => {
                Reply::Text(capability(tls_active, starttls_available, None))
            }
            (Phase::Authenticated { owner }, Command::Capability) => {
                Reply::Text(capability(tls_active, starttls_available, Some(owner)))
            }
            (Phase::Plain, Command::StartTls) if starttls_available && !tls_active => {
                Reply::StartTls
            }
            (Phase::Plain, Command::StartTls)
            | (Phase::Authenticated { .. }, Command::StartTls) => {
                no("TLS is unavailable or already active")
            }
            (Phase::Authenticated { .. }, Command::Unauthenticate) => {
                self.phase = Phase::Plain;
                self.scram_exchange = None;
                Reply::Unauthenticated(capability_lines(
                    tls_active,
                    starttls_available,
                    None,
                    b"OK \"unauthenticated\"\r\n",
                ))
            }
            (Phase::Plain, Command::Unauthenticate) => no("invalid command for state"),
            (Phase::Plain, Command::AuthenticatePlain(Some(response))) if tls_active => {
                Reply::Authenticate { response }
            }
            (Phase::Plain, Command::AuthenticatePlain(None)) if tls_active => {
                self.phase = Phase::AwaitingPlainResponse;
                Reply::AuthenticateChallenge
            }
            (Phase::Plain, Command::AuthenticatePlain(_)) => no("encryption required"),
            (Phase::Plain, Command::AuthenticateScramSha1(Some(response))) if tls_active => {
                self.phase = Phase::AwaitingScramVerifier;
                Reply::ScramClientFirst { response }
            }
            (Phase::Plain, Command::AuthenticateScramSha1(None)) if tls_active => {
                self.phase = Phase::AwaitingScramFirstResponse;
                Reply::AuthenticateChallenge
            }
            (Phase::Plain, Command::AuthenticateScramSha1(_)) => no("encryption required"),
            (Phase::AwaitingPlainResponse, Command::AuthenticateResponse(response)) => {
                self.phase = Phase::Plain;
                match SaslResponse::parse(response) {
                    SaslResponse::Cancel => no("authentication cancelled"),
                    SaslResponse::Payload(response) => Reply::Authenticate { response },
                }
            }
            (Phase::AwaitingScramFirstResponse, Command::AuthenticateResponse(response)) => {
                self.scram_first_response(response)
            }
            (Phase::AwaitingScramFinal, Command::AuthenticateResponse(response)) => {
                self.scram_final_response(response)
            }
            (Phase::AwaitingScramFirstResponse, Command::Invalid)
            | (Phase::AwaitingScramFinal, Command::Invalid) => {
                self.scram_failed();
                no("invalid SASL response")
            }
            (Phase::AwaitingPlainResponse, Command::Invalid) => {
                self.phase = Phase::Plain;
                no("syntax error")
            }
            (Phase::Plain, Command::Logout) | (Phase::Authenticated { .. }, Command::Logout) => {
                self.phase = Phase::Closed;
                ok("bye")
            }
            (Phase::Authenticated { .. }, Command::HaveSpace { name, size }) => {
                Reply::HaveSpace { name, size }
            }
            (Phase::Authenticated { .. }, Command::PutScript { name, source }) => {
                Reply::Put { name, source }
            }
            (Phase::Authenticated { .. }, Command::ListScripts) => Reply::List,
            (Phase::Authenticated { .. }, Command::SetActive(name)) => Reply::SetActive(name),
            (Phase::Authenticated { .. }, Command::GetScript(name)) => Reply::Get(name),
            (Phase::Authenticated { .. }, Command::DeleteScript(name)) => Reply::Delete(name),
            (Phase::Authenticated { .. }, Command::RenameScript { old, new }) => {
                Reply::Rename { old, new }
            }
            (Phase::Authenticated { .. }, Command::CheckScript(source)) => Reply::Check(source),
            (Phase::Plain, Command::Noop(tag))
            | (Phase::Authenticated { .. }, Command::Noop(tag)) => noop(tag),
            (_, Command::Invalid) => no("syntax error"),
            _ => no("invalid command for state"),
        }
    }

    /// Сообщает reactor-у, какая SASL-фаза владеет следующей строкой.
    pub fn awaiting_plain_response(&self) -> bool {
        matches!(self.phase, Phase::AwaitingPlainResponse)
    }

    /// Управляет framing client payload для PLAIN и SCRAM механизмов.
    pub fn awaiting_authentication_response(&self) -> bool {
        matches!(
            self.phase,
            Phase::AwaitingPlainResponse
                | Phase::AwaitingScramFirstResponse
                | Phase::AwaitingScramFinal
        )
    }

    /// Подхватывает verifier и переводит обмен к проверке client-final.
    pub fn scram_server_first(
        &mut self,
        credential: crate::sasl::scram_sha1_record::ScramSha1Record,
    ) -> std::io::Result<Reply> {
        let Some(exchange) = self.scram_exchange.as_mut() else {
            return Err(std::io::Error::other("SCRAM exchange is missing"));
        };
        let server_first = crate::sasl::scram_server_first::ServerFirst::create(
            &exchange.client_first,
            &credential,
        )?;
        let response = crate::sasl::encode_base64(server_first.message.as_bytes());
        exchange.server_first = Some(server_first);
        exchange.credential = Some(credential);
        self.phase = Phase::AwaitingScramFinal;
        Ok(Reply::ScramChallenge { response })
    }

    /// Привязывает parsed client-first к каноническому локальному account.
    pub fn begin_scram(
        &mut self,
        owner: String,
        client_first: crate::sasl::scram_client_first::ClientFirst,
    ) -> bool {
        match self.phase {
            Phase::AwaitingScramVerifier => {
                self.scram_exchange = Some(super::ScramExchange {
                    owner,
                    client_first,
                    server_first: None,
                    credential: None,
                });
                true
            }
            _ => false,
        }
    }

    /// Сбрасывает exchange после malformed frame, storage ошибки или cancel.
    pub fn scram_failed(&mut self) {
        self.scram_exchange = None;
        self.phase = Phase::Plain;
    }

    /// Завершает mutual proof и выбирает только typed success/error outcome.
    fn finish_scram(&mut self, response: &str) -> Reply {
        let Some(exchange) = self.scram_exchange.take() else {
            self.phase = Phase::Plain;
            return no("authentication failed");
        };
        let result = super::super::scram_final::verify(&exchange, response);
        self.phase = Phase::Plain;
        match result {
            super::super::scram_final::Outcome::Verified(server_final) => {
                self.phase = Phase::Authenticated {
                    owner: exchange.owner.clone(),
                };
                let final_data = crate::sasl::encode_base64(server_final.as_bytes());
                let ending = format!("OK (SASL \"{final_data}\") \"authenticated\"\r\n");
                let response =
                    super::capability_lines(true, false, Some(&exchange.owner), ending.as_bytes());
                Reply::ScramAccepted {
                    owner: exchange.owner,
                    response,
                }
            }
            super::super::scram_final::Outcome::Rejected(server_final) => Reply::ScramRejected {
                response: crate::sasl::encode_base64(server_final.as_bytes()),
            },
        }
    }

    /// Завершает PLAIN verification или явно отказывает для текущего профиля.
    pub fn authenticated(&mut self, owner: String, accepted: bool, tls_active: bool) -> Reply {
        match (&self.phase, accepted, tls_active) {
            (Phase::Plain, true, true) => {
                self.phase = Phase::Authenticated {
                    owner: owner.clone(),
                };
                Reply::Text(super::capability_lines(
                    tls_active,
                    false,
                    Some(&owner),
                    b"OK \"authenticated\"\r\n",
                ))
            }
            (Phase::Plain, true, false) => no("encryption required"),
            (Phase::Plain, false, _) => no("authentication failed"),
            _ => no("no authentication pending"),
        }
    }

    fn scram_first_response(&mut self, response: String) -> Reply {
        match SaslResponse::parse(response) {
            SaslResponse::Cancel => {
                self.scram_failed();
                no("authentication cancelled")
            }
            SaslResponse::Payload(response) => {
                self.phase = Phase::AwaitingScramVerifier;
                Reply::ScramClientFirst { response }
            }
        }
    }

    fn scram_final_response(&mut self, response: String) -> Reply {
        match SaslResponse::parse(response) {
            SaslResponse::Cancel => {
                self.scram_failed();
                no("authentication cancelled")
            }
            SaslResponse::Payload(response) => self.finish_scram(&response),
        }
    }
}
