//! Переход IMAP-сессии из authenticated в selected state.

use super::*;

/// Читает INBOX текущего account и меняет state только после completion.
#[allow(clippy::too_many_arguments)]
pub(super) fn queue(
    session: &mut Session,
    tag: String,
    requested_mailbox: imap::MailboxName,
    read_only: bool,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if let imap::MailboxName::Named(_) = requested_mailbox {
        return session.push(format!("{tag} NO mailbox does not exist\r\n").as_bytes());
    }
    let Some(mailbox) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: mailbox.clone(),
        },
        Pending::ReadForSelect {
            tag,
            mailbox,
            read_only,
        },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
