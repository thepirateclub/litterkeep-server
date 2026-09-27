//! Собственное ManageSieve ядро: commands, states и storage-neutral replies.

#[path = "managesieve/command.rs"]
mod command;
#[path = "managesieve/session.rs"]
mod session;
#[cfg(test)]
#[path = "managesieve/tests.rs"]
mod tests;

pub use command::{Command, parse, parse_authentication_response};
pub use session::{Reply, Session, greeting};
