//! POP3 AUTHORIZATION, TRANSACTION и UPDATE без доступа к Store.

use super::{Command, Maildrop, SaslMechanism};
use crate::store::Message;

#[path = "session_sasl.rs"]
mod sasl;

#[derive(Default)]
enum Phase {
    #[default]
    Authorization,
    UserAccepted,
    SaslPlainResponse,
    Transaction,
    Update,
}

/// Состояние AUTHORIZATION, которое требуется восстановить после отказа SASL.
enum PriorAuthorization {
    NoUser,
    User(String),
}

pub enum Reply {
    Ok(String),
    Error(&'static str),
    ErrorCode(ResponseCode, &'static str),
    SaslContinue,
    Multiline(Vec<u8>),
    Message(Vec<u8>),
    Authenticate { username: String, password: Vec<u8> },
    CommitDeletes(Vec<u64>),
    StartTls,
}

/// Стандартизованный RFC 2449 код причины POP3-ошибки.
pub enum ResponseCode {
    InUse,
}
#[derive(Default)]
pub struct Session {
    phase: Phase,
    username: Option<String>,
    sasl_previous_authorization: Option<PriorAuthorization>,
    maildrop: Option<Maildrop>,
    tls_active: bool,
}
impl Session {
    pub fn apply(&mut self, command: Command) -> Reply {
        match (&self.phase, command) {
            (Phase::Authorization, Command::User(name)) => {
                self.username = Some(name);
                self.phase = Phase::UserAccepted;
                Reply::Ok("user accepted".into())
            }
            (Phase::UserAccepted, Command::User(_)) => {
                self.reset_authorization();
                Reply::Error("invalid command for state")
            }
            (Phase::UserAccepted, Command::Pass(password)) => self.request_authentication(password),
            (Phase::Authorization, Command::Pass(_)) => Reply::Error("USER required"),
            (
                Phase::Authorization | Phase::UserAccepted,
                Command::Auth {
                    mechanism,
                    initial_response,
                },
            ) => self.begin_authentication(mechanism, initial_response),
            (Phase::Authorization | Phase::UserAccepted | Phase::Transaction, Command::Capa) => {
                capa_reply(self.tls_active)
            }
            (Phase::Authorization | Phase::UserAccepted, Command::Stls) => Reply::StartTls,
            (Phase::Authorization | Phase::UserAccepted, Command::Noop) => Reply::Ok("noop".into()),
            (Phase::Transaction, Command::Stat) => self.stat(),
            (Phase::Transaction, Command::List(number)) => self.list(number),
            (Phase::Transaction, Command::Uidl(number)) => self.uidl(number),
            (Phase::Transaction, Command::Retr(number)) => self.retrieve(number),
            (Phase::Transaction, Command::Top { number, lines }) => self.top(number, lines),
            (Phase::Transaction, Command::Dele(number)) => self.delete(number),
            (Phase::Transaction, Command::Noop) => Reply::Ok("noop".into()),
            (Phase::Transaction, Command::Rset) => self.reset(),
            (Phase::Transaction, Command::Quit) => self.request_commit(),
            (_, Command::Quit) => {
                self.phase = Phase::Update;
                Reply::Ok("bye".into())
            }
            _ => Reply::Error("invalid command for state"),
        }
    }
    pub fn authenticated(&mut self, messages: Vec<Message>) -> Reply {
        match (&self.phase, self.username.as_ref()) {
            (Phase::UserAccepted, Some(_)) => {
                self.sasl_previous_authorization = None;
                self.maildrop = Some(Maildrop::new(messages));
                self.phase = Phase::Transaction;
                Reply::Ok("authenticated".into())
            }
            _ => Reply::Error("USER required"),
        }
    }
    /// Сбрасывает незавершённую пару USER/PASS после отказа входа или STLS.
    pub fn reset_authorization(&mut self) {
        self.username = None;
        self.sasl_previous_authorization = None;
        self.phase = Phase::Authorization;
    }
    pub fn commit_deletes(&mut self, accepted: bool) -> Reply {
        match (self.phase_is_transaction(), accepted) {
            (true, true) => {
                self.phase = Phase::Update;
                Reply::Ok("bye".into())
            }
            (true, false) => Reply::Error("delete commit failed"),
            (false, _) => Reply::Error("no pending update"),
        }
    }
    fn request_authentication(&self, password: String) -> Reply {
        match self.username.as_ref() {
            Some(username) if !password.is_empty() => Reply::Authenticate {
                username: username.clone(),
                password: password.into_bytes(),
            },
            _ => Reply::Error("USER required"),
        }
    }
    fn stat(&self) -> Reply {
        self.drop()
            .map_or(Reply::Error("maildrop unavailable"), |drop| {
                let (count, bytes) = drop.stat();
                Reply::Ok(format!("{count} {bytes}"))
            })
    }
    fn list(&self, number: Option<u32>) -> Reply {
        match self.drop() {
            Some(drop) => drop.list(number).map_or_else(Reply::Error, list_reply),
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn uidl(&self, number: Option<u32>) -> Reply {
        match self.drop() {
            Some(drop) => drop.uidl(number).map_or_else(Reply::Error, uidl_reply),
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn retrieve(&self, number: u32) -> Reply {
        match self.drop() {
            Some(drop) => drop.retrieve(number).map_or_else(Reply::Error, |bytes| {
                message_reply("message follows", bytes)
            }),
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn top(&self, number: u32, lines: u32) -> Reply {
        match self.drop() {
            Some(drop) => drop.top(number, lines).map_or_else(Reply::Error, |bytes| {
                message_reply("top of message follows", bytes)
            }),
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn delete(&mut self, number: u32) -> Reply {
        match self.drop_mut() {
            Some(drop) => drop
                .delete(number)
                .map_or_else(Reply::Error, |_| Reply::Ok("marked".into())),
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn reset(&mut self) -> Reply {
        match self.drop_mut() {
            Some(drop) => {
                drop.reset();
                Reply::Ok("reset".into())
            }
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn request_commit(&self) -> Reply {
        self.drop()
            .map_or(Reply::Error("maildrop unavailable"), |drop| {
                Reply::CommitDeletes(drop.deleted_uids())
            })
    }
    fn drop(&self) -> Option<&Maildrop> {
        self.maildrop.as_ref()
    }
    fn drop_mut(&mut self) -> Option<&mut Maildrop> {
        self.maildrop.as_mut()
    }
    fn phase_is_transaction(&self) -> bool {
        matches!(self.phase, Phase::Transaction)
    }
}
fn capa_reply(tls_active: bool) -> Reply {
    let mut response = b"+OK Capability list\r\nUSER\r\nUIDL\r\nTOP\r\nRESP-CODES\r\n".to_vec();
    if tls_active {
        response.extend_from_slice(b"SASL PLAIN\r\n");
    } else {
        response.extend_from_slice(b"STLS\r\n");
    }
    response.extend_from_slice(b".\r\n");
    Reply::Multiline(response)
}
fn list_reply(entries: Vec<(u32, usize)>) -> Reply {
    let mut output = b"+OK scan listing follows\r\n".to_vec();
    for (number, size) in entries {
        output.extend_from_slice(format!("{number} {size}\r\n").as_bytes());
    }
    output.extend_from_slice(b".\r\n");
    Reply::Multiline(output)
}
fn uidl_reply(entries: Vec<(u32, String)>) -> Reply {
    let mut output = b"+OK unique-id listing follows\r\n".to_vec();
    for (number, uid) in entries {
        output.extend_from_slice(format!("{number} {uid}\r\n").as_bytes());
    }
    output.extend_from_slice(b".\r\n");
    Reply::Multiline(output)
}

/// Добавляет обязательный POP3 +OK frame перед уже dot-stuffed RFC822 bytes.
fn message_reply(description: &str, bytes: Vec<u8>) -> Reply {
    let mut output = format!("+OK {description}\r\n").into_bytes();
    output.extend_from_slice(&bytes);
    Reply::Message(output)
}
