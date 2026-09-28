//! Переходы SASL PLAIN отделены от команд POP3 maildrop.

use super::*;

impl Session {
    /// Переключает SASL capabilities после завершения TLS handshake.
    pub fn tls_established(&mut self) {
        self.tls_active = true;
        self.reset_authorization();
    }

    /// Проверяет, ожидается ли SASL response вместо следующей POP3 команды.
    pub fn awaiting_sasl_response(&self) -> bool {
        matches!(
            self.phase,
            Phase::SaslPlainResponse
                | Phase::SaslScramFirst
                | Phase::SaslScramFinal
                | Phase::SaslScramAcknowledgement
        )
    }

    /// Обрабатывает continuation response или RFC 5034 cancellation marker.
    pub fn sasl_plain_response(&mut self, response: &str) -> Reply {
        match self.phase {
            Phase::SaslScramFirst => {
                return match super::session_scram::ClientResponse::parse(response) {
                    super::session_scram::ClientResponse::Cancel => {
                        self.restore_authorization_after_sasl();
                        Reply::Error("authentication cancelled")
                    }
                    super::session_scram::ClientResponse::Payload(payload) => {
                        super::session_scram::client_first(self, payload)
                    }
                };
            }
            Phase::SaslScramFinal => {
                return match super::session_scram::ClientResponse::parse(response) {
                    super::session_scram::ClientResponse::Cancel => {
                        self.restore_authorization_after_sasl();
                        Reply::Error("authentication cancelled")
                    }
                    super::session_scram::ClientResponse::Payload(payload) => {
                        super::session_scram::client_final(self, payload)
                    }
                };
            }
            Phase::SaslScramAcknowledgement => {
                return super::session_scram::acknowledgement(self, response);
            }
            _ => {}
        }
        match (&self.phase, response) {
            (Phase::SaslPlainResponse, "*") => {
                self.restore_authorization_after_sasl();
                Reply::Error("authentication cancelled")
            }
            (Phase::SaslPlainResponse, _) => self.plain_credentials(response),
            _ => Reply::Error("no SASL response is pending"),
        }
    }

    /// Восстанавливает состояние до AUTH при отказе проверки credentials.
    pub fn authentication_rejected(&mut self) -> Reply {
        self.restore_authorization_after_sasl();
        Reply::Error("authentication failed")
    }

    /// Завершает успешную проверку credentials до получения maildrop.
    pub fn authentication_accepted(&mut self) {
        self.sasl_previous_authorization = None;
    }

    /// Возвращает typed error после успешного входа и конфликта maildrop.
    pub fn maildrop_in_use(&mut self) -> Reply {
        self.restore_authorization_after_sasl();
        Reply::ErrorCode(ResponseCode::InUse, "maildrop is locked")
    }

    /// Проверяет transport policy и запускает поддерживаемый SASL механизм.
    pub(super) fn begin_authentication(
        &mut self,
        mechanism: SaslMechanism,
        initial_response: Option<SaslInitialResponse>,
    ) -> Reply {
        if !self.tls_active {
            return Reply::Error("STLS required before AUTH PLAIN");
        }
        match mechanism {
            SaslMechanism::Plain => self.start_plain_authentication(initial_response),
            SaslMechanism::ScramSha1 => super::session_scram::start(self, initial_response),
            SaslMechanism::Unsupported => Reply::Error("unsupported SASL mechanism"),
        }
    }

    fn plain_credentials(&mut self, response: &str) -> Reply {
        match crate::sasl::parse_plain(response) {
            Ok(credentials) => match String::from_utf8(credentials.password) {
                Ok(password) => {
                    self.username = Some(credentials.account.clone());
                    self.phase = Phase::UserAccepted;
                    Reply::Authenticate {
                        username: credentials.account,
                        password: password.into_bytes(),
                    }
                }
                Err(_) => self.invalid_plain_credentials(),
            },
            Err(_) => self.invalid_plain_credentials(),
        }
    }

    fn invalid_plain_credentials(&mut self) -> Reply {
        self.restore_authorization_after_sasl();
        Reply::Error("invalid AUTH PLAIN credentials")
    }

    fn start_plain_authentication(
        &mut self,
        initial_response: Option<SaslInitialResponse>,
    ) -> Reply {
        self.sasl_previous_authorization = Some(match &self.username {
            Some(username) => PriorAuthorization::User(username.clone()),
            None => PriorAuthorization::NoUser,
        });
        match initial_response {
            Some(SaslInitialResponse::Empty) => self.plain_credentials(""),
            Some(SaslInitialResponse::Encoded(response)) => self.plain_credentials(&response),
            None => {
                self.phase = Phase::SaslPlainResponse;
                Reply::SaslContinue
            }
        }
    }

    pub(super) fn restore_authorization_after_sasl(&mut self) {
        self.scram_exchange = None;
        match self.sasl_previous_authorization.take() {
            Some(PriorAuthorization::User(username)) => {
                self.username = Some(username);
                self.phase = Phase::UserAccepted;
            }
            Some(PriorAuthorization::NoUser) => {
                self.username = None;
                self.phase = Phase::Authorization;
            }
            None => self.reset_authorization(),
        }
    }
}
