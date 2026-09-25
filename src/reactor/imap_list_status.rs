//! Формирование STATUS data ответа, запрошенного командой LIST.

use super::{Session, imap};
use std::io;

/// Пишет untagged STATUS после LIST из заранее построенного Store snapshot.
pub(super) fn write(
    session: &mut Session,
    mailbox: &str,
    status: &crate::store::MailboxStatus,
    attributes: &[imap::status::StatusAttribute],
) -> io::Result<()> {
    let values = attributes
        .iter()
        .filter_map(|attribute| status_value(*attribute, status))
        .collect::<Vec<_>>()
        .join(" ");
    session.push(format!("* STATUS {mailbox} ({values})\r\n").as_bytes())
}

/// Сериализует запрошенное поле без повторного чтения Store или blob.
fn status_value(
    attribute: imap::status::StatusAttribute,
    status: &crate::store::MailboxStatus,
) -> Option<String> {
    match attribute {
        imap::status::StatusAttribute::Messages => Some(format!("MESSAGES {}", status.messages)),
        imap::status::StatusAttribute::UidNext => Some(format!("UIDNEXT {}", status.uid_next)),
        imap::status::StatusAttribute::UidValidity => {
            Some(format!("UIDVALIDITY {}", status.uid_validity))
        }
        imap::status::StatusAttribute::Unseen => Some(format!("UNSEEN {}", status.unseen)),
        imap::status::StatusAttribute::Deleted => Some(format!("DELETED {}", status.deleted)),
        imap::status::StatusAttribute::Size => status.size.map(|size| format!("SIZE {size}")),
        imap::status::StatusAttribute::AppendLimit => {
            Some(format!("APPENDLIMIT {}", crate::smtp::MAX_MESSAGE_SIZE))
        }
        imap::status::StatusAttribute::Recent => None,
    }
}
