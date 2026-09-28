//! Собственное ManageSieve ядро: commands, states и storage-neutral replies.

#[path = "managesieve/command.rs"]
mod command;
#[path = "managesieve/scram_final.rs"]
mod scram_final;
#[cfg(test)]
#[path = "managesieve/scram_session_tests.rs"]
mod scram_session_tests;
#[path = "managesieve/session.rs"]
mod session;
#[cfg(test)]
#[path = "managesieve/tests.rs"]
mod tests;

pub use command::{Command, SaslMechanism, SaslResponse, parse, parse_authentication_response};
pub use session::{Reply, Session, greeting};
