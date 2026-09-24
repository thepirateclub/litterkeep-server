use crate::{
    mail_date::MailDate,
    mail_flags::{KeywordFlag, MessageFlags},
};
use std::{collections::BTreeSet, io, path::PathBuf};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub uid: u64,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub flags: MessageFlags,
    /// Расширяемые пользовательские IMAP flags из того же durable snapshot.
    pub keywords: BTreeSet<KeywordFlag>,
    pub internal_date: MailDate,
}

/// Согласованный снимок mailbox, прочитанный одним владельцем store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxSnapshot {
    pub uid_validity: u32,
    pub uid_next: u64,
    pub messages: Vec<Message>,
}
#[derive(Clone, Debug)]
pub struct Record {
    pub uid: u64,
    pub transaction_id: u64,
    /// Transaction immutable blob. COPY сохраняет его отдельно от transaction
    /// ссылки, чтобы не копировать payload и не подменять имя файла.
    pub blob_transaction_id: u64,
    pub name: String,
    pub flags: MessageFlags,
    /// Начальные keywords, фиксируемые в версии 4 mailbox journal.
    pub keywords: BTreeSet<KeywordFlag>,
    pub internal_date: MailDate,
}

/// Начальные метаданные одной mailbox entry, фиксируемые вместе с UID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendMetadata {
    pub flags: MessageFlags,
    pub keywords: BTreeSet<KeywordFlag>,
    pub internal_date: MailDate,
}

impl Default for AppendMetadata {
    fn default() -> Self {
        Self {
            flags: MessageFlags::default(),
            keywords: BTreeSet::new(),
            internal_date: MailDate::now(),
        }
    }
}
pub fn validate_mailbox(mailbox: &str) -> io::Result<()> {
    let characters_are_safe = mailbox
        .bytes()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'));
    match (mailbox.is_empty(), characters_are_safe) {
        (true, _) | (false, false) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe mailbox",
        )),
        (false, true) => Ok(()),
    }
}
pub fn blob_name(transaction_id: u64) -> String {
    format!("message-{transaction_id}.eml")
}
