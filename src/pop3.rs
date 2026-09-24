//! Собственное POP3 ядро: grammar, состояния и session-local maildrop.
//!
//! Этот модуль не владеет Store и не выполняет сетевой ввод. Reactor передаёт
//! ему только typed command и snapshot общего INBOX после успешной AUTH.

#[path = "pop3/command.rs"]
mod command;
#[path = "pop3/maildrop.rs"]
mod maildrop;
#[path = "pop3/session.rs"]
mod session;

pub use command::{Command, MAX_COMMAND_LINE_SIZE, parse};
pub use maildrop::Maildrop;
pub use session::{Reply, Session};

#[cfg(test)]
#[path = "pop3/tests.rs"]
mod tests;
