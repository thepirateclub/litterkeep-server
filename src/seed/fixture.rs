//! Типизированная модель и объединение детерминированных seed fixtures.

#[path = "fixture_attachment.rs"]
mod attachment;
#[path = "fixture_basic.rs"]
mod basic;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Delivery {
    Smtp,
    ImapAppend,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mailbox {
    Inbox,
    Sent,
    Drafts,
    Archive,
}

impl Mailbox {
    pub(super) fn wire(self) -> &'static str {
        match self {
            Self::Inbox => "INBOX",
            Self::Sent => "Sent",
            Self::Drafts => "Drafts",
            Self::Archive => "Archive",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Owner {
    Alice,
    Bob,
}

pub(super) struct Fixture {
    pub id: String,
    pub recipient: String,
    pub mailbox: Mailbox,
    pub owner: Owner,
    pub delivery: Delivery,
    pub flags: String,
    pub message: Vec<u8>,
}

/// Возвращает полный bounded набор из 35 сообщений, включая 10 MiB attachment.
pub(super) fn all() -> Vec<Fixture> {
    let mut result = basic::fixtures();
    result.extend(attachment::fixtures());
    result
}

pub(super) fn message(id: &str, subject: &str, extra: &str, media: &str, body: &[u8]) -> Vec<u8> {
    let mut result = format!(
        "From: Alice <alice@litterkeep.test>\r\nTo: Bob <bob@litterkeep.test>\r\nSubject: {subject}\r\nX-Litterkeep-Fixture: {id}\r\nMessage-ID: <{id}@litterkeep.test>\r\nMIME-Version: 1.0\r\n{extra}Content-Type: {media}\r\n\r\n"
    ).into_bytes();
    result.extend_from_slice(body);
    result
}

pub(super) fn fixture(
    id: &str,
    mailbox: Mailbox,
    owner: Owner,
    delivery: Delivery,
    flags: &str,
    message: Vec<u8>,
) -> Fixture {
    let recipient = match owner {
        Owner::Alice => "alice@litterkeep.test",
        Owner::Bob => "bob@litterkeep.test",
    };
    Fixture {
        id: id.to_owned(),
        recipient: recipient.to_owned(),
        mailbox,
        owner,
        delivery,
        flags: flags.to_owned(),
        message,
    }
}
