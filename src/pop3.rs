//! Собственное POP3 ядро: grammar, состояния и session-local maildrop.
//!
//! Этот модуль не владеет Store и не выполняет сетевой ввод. Reactor передаёт
//! ему только typed command и snapshot общего INBOX после успешной AUTH.

#[path = "pop3/command.rs"]
mod command;
#[path = "pop3/language.rs"]
mod language;
#[path = "pop3/listing_reply.rs"]
mod listing_reply;
#[path = "pop3/maildrop.rs"]
mod maildrop;
#[cfg(any(target_os = "linux", test))]
#[path = "pop3/reply_wire.rs"]
pub(crate) mod reply_wire;
#[path = "pop3/session.rs"]
mod session;

pub use command::{Command, MAX_COMMAND_LINE_SIZE, SaslMechanism, parse};
pub use language::LanguageRequest;
pub use maildrop::Maildrop;
pub use session::{Reply, ResponseCode, Session};

#[cfg(test)]
#[path = "pop3/tests.rs"]
mod tests;
