//! Общая постановка read-only команд выбранного IMAP mailbox.

use super::*;

/// Ставит чтение только после worker-подтверждённой selected session.
#[allow(clippy::too_many_arguments)]
pub(super) fn queue_authenticated_read<F>(
    session: &mut Session,
    tag: String,
    pending: F,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()>
where
    F: FnOnce(String, String) -> Pending,
{
    let Some(mailbox) = session.selected_mailbox.clone() else {
        return session.push(format!("{tag} NO select mailbox first\r\n").as_bytes());
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: mailbox.clone(),
        },
        pending(tag, mailbox),
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}
