//! POP3 AUTHORIZATION, TRANSACTION и UPDATE без доступа к Store.

use super::listing_reply::{list_reply, uidl_reply};
use super::{Command, DeleteError, LanguageRequest, Maildrop, SaslInitialResponse, SaslMechanism};
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

/// Стандартизованные RFC 2449 и RFC 6856 коды причины POP3-ошибки.
pub enum ResponseCode {
    InUse,
    Utf8,
}
#[derive(Default)]
pub struct Session {
    phase: Phase,
    username: Option<String>,
    sasl_previous_authorization: Option<PriorAuthorization>,
    maildrop: Option<Maildrop>,
    tls_active: bool,
    /// Отмечает разрешение передавать UTF-8 строки после команды UTF8.
    utf8_active: bool,
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
            (Phase::Authorization | Phase::UserAccepted, Command::Stls) => match self.utf8_active {
                true => Reply::Error("STLS must precede UTF8"),
                false => Reply::StartTls,
            },
            (Phase::Authorization | Phase::UserAccepted, Command::Utf8) => {
                self.utf8_active = true;
                Reply::Ok("UTF-8 mode enabled".into())
            }
            (
                Phase::Authorization | Phase::UserAccepted | Phase::Transaction,
                Command::Lang(language),
            ) => lang_reply(language),
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
        match self.utf8_policy(number) {
            Ok(()) => match self.drop() {
                Some(drop) => drop
                    .list(number)
                    .map_or_else(Reply::Error, |entries| list_reply(number, entries)),
                None => Reply::Error("maildrop unavailable"),
            },
            Err(reply) => reply,
        }
    }
    fn uidl(&self, number: Option<u32>) -> Reply {
        match self.drop() {
            Some(drop) => drop
                .uidl(number)
                .map_or_else(Reply::Error, |entries| uidl_reply(number, entries)),
            None => Reply::Error("maildrop unavailable"),
        }
    }
    fn retrieve(&self, number: u32) -> Reply {
        match self.retrieval_policy(number) {
            Ok(()) => match self.drop() {
                Some(drop) => drop.retrieve(number).map_or_else(Reply::Error, |bytes| {
                    message_reply("message follows", bytes)
                }),
                None => Reply::Error("maildrop unavailable"),
            },
            Err(reply) => reply,
        }
    }
    fn top(&self, number: u32, lines: u32) -> Reply {
        match self.retrieval_policy(number) {
            Ok(()) => match self.drop() {
                Some(drop) => drop.top(number, lines).map_or_else(Reply::Error, |bytes| {
                    message_reply("top of message follows", bytes)
                }),
                None => Reply::Error("maildrop unavailable"),
            },
            Err(reply) => reply,
        }
    }
    fn utf8_policy(&self, number: Option<u32>) -> Result<(), Reply> {
        match self.drop() {
            Some(drop) => super::text_policy::evaluate(drop, number, self.utf8_active),
            None => Err(Reply::Error("maildrop unavailable")),
        }
    }
    /// Проверяет ограничения POP3 transfer encoding до чтения сообщения.
    fn retrieval_policy(&self, number: u32) -> Result<(), Reply> {
        match self.drop() {
            Some(drop) => match drop.transfer_policy(number) {
                Ok(super::message_profile::MimeTransferPolicy::Binary) => Err(Reply::Error(
                    "MIME binary transfer encoding is not permitted",
                )),
                Ok(super::message_profile::MimeTransferPolicy::Allowed) => {
                    self.utf8_policy(Some(number))
                }
                Ok(super::message_profile::MimeTransferPolicy::Unverifiable) => {
                    Err(Reply::Error("MIME structure cannot be verified for POP3"))
                }
                Err(error) => Err(Reply::Error(error)),
            },
            None => Err(Reply::Error("maildrop unavailable")),
        }
    }
    fn delete(&mut self, number: u32) -> Reply {
        match self.drop_mut() {
            Some(drop) => match drop.delete(number) {
                Ok(()) => Reply::Ok("marked".into()),
                Err(DeleteError::AlreadyDeleted) => Reply::Error("message already deleted"),
                Err(DeleteError::NoSuchMessage) => Reply::Error("no such message"),
            },
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
    /// Сообщает reactor-у, что QUIT завершил POP3 UPDATE.
    pub fn is_update(&self) -> bool {
        matches!(self.phase, Phase::Update)
    }
}
/// Формирует стабильный RFC 2449 набор и объявляет UTF8 в обоих состояниях.
/// Параметр USER объявляется только вместе с реальным UTF-8 USER/PASS parser-ом.
fn capa_reply(tls_active: bool) -> Reply {
    let mut response =
        b"+OK Capability list\r\nUSER\r\nUIDL\r\nTOP\r\nRESP-CODES\r\nUTF8 USER\r\nLANG\r\n"
            .to_vec();
    if tls_active {
        response.extend_from_slice(b"SASL PLAIN\r\n");
    } else {
        response.extend_from_slice(b"STLS\r\n");
    }
    response.extend_from_slice(b".\r\n");
    Reply::Multiline(response)
}
/// Возвращает поддерживаемую локаль либо RFC 6856 language listing.
fn lang_reply(language: LanguageRequest) -> Reply {
    match language {
        LanguageRequest::Listing => {
            Reply::Multiline(b"+OK Language listing follows\r\nen English\r\n.\r\n".to_vec())
        }
        LanguageRequest::Preferred | LanguageRequest::English => {
            Reply::Ok("en Language selected".into())
        }
        LanguageRequest::Unsupported => Reply::Error("language not supported"),
    }
}
/// Добавляет обязательный POP3 +OK frame перед уже dot-stuffed RFC822 bytes.
fn message_reply(description: &str, bytes: Vec<u8>) -> Reply {
    let mut output = format!("+OK {description}\r\n").into_bytes();
    output.extend_from_slice(&bytes);
    Reply::Message(output)
}
