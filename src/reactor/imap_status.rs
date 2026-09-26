//! STATUS читает согласованный snapshot, не переводя сессию в selected state.

use super::*;
use crate::imap::status::StatusAttribute;

#[allow(clippy::too_many_arguments)]
pub(super) fn queue(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    attributes: Vec<StatusAttribute>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let name = match mailbox {
        imap::MailboxName::Inbox => "INBOX".to_owned(),
        imap::MailboxName::Named(name) => name,
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::ReadFolder {
            request_id: id,
            account,
            name: name.clone(),
        },
        Pending::Status {
            tag,
            name,
            attributes,
        },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

pub(super) fn completed(
    session: &mut Session,
    tag: String,
    name: String,
    attributes: Vec<StatusAttribute>,
    result: io::Result<crate::store::ResolvedMailbox>,
) -> io::Result<()> {
    let resolved = match result {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return session.push(format!("{tag} NO [NONEXISTENT] STATUS failed\r\n").as_bytes());
        }
        Err(_) => return session.push(format!("{tag} NO STATUS failed\r\n").as_bytes()),
    };
    let recent_count = resolved.recent_uids.len();
    let snapshot = resolved.snapshot;
    let mut values = Vec::with_capacity(attributes.len());
    for attribute in attributes {
        let value = match attribute {
            StatusAttribute::Messages => format!("MESSAGES {}", snapshot.messages.len()),
            StatusAttribute::UidNext => format!("UIDNEXT {}", snapshot.uid_next),
            StatusAttribute::UidValidity => format!("UIDVALIDITY {}", snapshot.uid_validity),
            StatusAttribute::Unseen => {
                let count = snapshot
                    .messages
                    .iter()
                    .filter(|message| !message.flags.contains(crate::mail_flags::SystemFlag::Seen))
                    .count();
                format!("UNSEEN {count}")
            }
            StatusAttribute::Recent => format!("RECENT {recent_count}"),
            StatusAttribute::Deleted => {
                let count = snapshot
                    .messages
                    .iter()
                    .filter(|message| {
                        message
                            .flags
                            .contains(crate::mail_flags::SystemFlag::Deleted)
                    })
                    .count();
                format!("DELETED {count}")
            }
            StatusAttribute::Size => {
                let size = snapshot
                    .messages
                    .iter()
                    .map(|message| message.bytes.len() as u64)
                    .sum::<u64>();
                format!("SIZE {size}")
            }
            StatusAttribute::AppendLimit => {
                format!("APPENDLIMIT {}", crate::smtp::MAX_MESSAGE_SIZE)
            }
        };
        values.push(value);
    }
    let mailbox = imap_folders::quote(&name, session.imap_string_mode);
    session.push(
        format!(
            "* STATUS {mailbox} ({})\r\n{tag} OK STATUS completed\r\n",
            values.join(" ")
        )
        .as_bytes(),
    )
}
