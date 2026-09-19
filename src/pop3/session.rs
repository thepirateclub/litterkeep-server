//! POP3 AUTHORIZATION, TRANSACTION и UPDATE без доступа к Store.

use super::{Command, Maildrop};
use crate::store::Message;

#[derive(Default)]
enum Phase {
    #[default]
    Authorization,
    Transaction,
    Update,
}
pub enum Reply {
    Ok(String),
    Error(&'static str),
    Multiline(Vec<u8>),
    Authenticate { username: String, password: String },
    CommitDeletes(Vec<u64>),
    StartTls,
}
#[derive(Default)]
pub struct Session {
    phase: Phase,
    username: Option<String>,
    maildrop: Option<Maildrop>,
}
impl Session {
    pub fn apply(&mut self, command: Command) -> Reply {
        match (&self.phase, command) {
            (Phase::Authorization, Command::User(name)) => {
                self.username = Some(name);
                Reply::Ok("user accepted".into())
            }
            (Phase::Authorization, Command::Pass(password)) => {
                self.request_authentication(password)
            }
            (Phase::Authorization, Command::Capa) => Reply::Multiline(
                b"+OK Capability list\r\nUSER\r\nUIDL\r\nTOP\r\nRESP-CODES\r\nSTLS\r\n.\r\n"
                    .to_vec(),
            ),
            (Phase::Authorization, Command::Stls) => Reply::StartTls,
            (Phase::Authorization, Command::Quit) => {
                self.phase = Phase::Update;
                Reply::Ok("bye".into())
            }
            (Phase::Transaction, Command::Stat) => self.stat(),
            (Phase::Transaction, Command::List(number)) => self.list(number),
            (Phase::Transaction, Command::Uidl(number)) => self.uidl(number),
            (Phase::Transaction, Command::Retr(number)) => self.retrieve(number),
            (Phase::Transaction, Command::Top { number, lines }) => self.top(number, lines),
            (Phase::Transaction, Command::Dele(number)) => self.delete(number),
            (Phase::Transaction, Command::Noop) => Reply::Ok("noop".into()),
            (Phase::Transaction, Command::Rset) => self.reset(),
            (Phase::Transaction, Command::Capa) => Reply::Multiline(
                b"+OK Capability list\r\nUSER\r\nUIDL\r\nTOP\r\nRESP-CODES\r\n.\r\n".to_vec(),
            ),
            (Phase::Transaction, Command::Quit) => self.request_commit(),
            (_, Command::Quit) => {
                self.phase = Phase::Update;
                Reply::Ok("bye".into())
            }
            _ => Reply::Error("invalid command for state"),
        }
    }
    pub fn authenticated(&mut self, messages: Vec<Message>) -> Reply {
        match self.username.as_ref() {
            Some(_) => {
                self.maildrop = Some(Maildrop::new(messages));
                self.phase = Phase::Transaction;
                Reply::Ok("authenticated".into())
            }
            None => Reply::Error("USER required"),
        }
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
                password,
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
    Reply::Multiline(output)
}
