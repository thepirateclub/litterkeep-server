//! Переход IMAP-сессии из authenticated в selected state.

use super::*;

/// Разрешает account-scoped имя и меняет state только после snapshot completion.
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
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    let name = match requested_mailbox {
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
            name,
        },
        Pending::ReadForSelect { tag, read_only },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
