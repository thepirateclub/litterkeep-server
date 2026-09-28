//! Владение и сброс временного ManageSieve SCRAM exchange.

use super::{Phase, ScramExchange, Session};

impl Session {
    /// Привязывает client-first к каноническому account до verifier lookup.
    pub fn begin_scram(
        &mut self,
        owner: String,
        client_first: crate::sasl::scram_client_first::ClientFirst,
    ) -> bool {
        match self.phase {
            Phase::AwaitingScramVerifier => {
                self.scram_exchange = Some(ScramExchange {
                    owner,
                    client_first,
                    server_first: None,
                    credential: None,
                    sha256_server_first: None,
                    sha256_credential: None,
                    channel_binding: None,
                });
                true
            }
            _ => false,
        }
    }

    /// Сбрасывает exchange после malformed frame, storage ошибки или отмены.
    pub fn scram_failed(&mut self) {
        self.scram_exchange = None;
        self.phase = Phase::Plain;
    }
}
