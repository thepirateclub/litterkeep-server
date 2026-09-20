//! Маршрутизация завершений единственного Store owner-а к session-владельцам.

use super::*;

pub(super) fn complete(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<SessionLease>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    next_request: &mut u64,
    maildrops: &mut MaildropLeases,
) -> io::Result<Vec<Response>> {
    let mut unclaimed = Vec::new();
    while let Ok(response) = worker.worker.try_response() {
        let id = completion_id::response_id(&response);
        let Some((slot, generation, pending)) = owners.remove(&id) else {
            // Scheduler и protocol session используют один Store owner. Ответ,
            // не принадлежащий session, caller маршрутизирует typed владельцу.
            unclaimed.push(response);
            continue;
        };
        super::imap_idle::notify_completion(sessions, &pending, &response);
        if !matches!(sessions.get(slot).and_then(Option::as_ref).map(|lease| lease.get().generation), Some(value) if value == generation)
        {
            continue;
        }
        if let Some(session) = sessions
            .get_mut(slot)
            .and_then(Option::as_mut)
            .map(SessionLease::get_mut)
        {
            session.pending = None;
        }
        if let Some((request, next_pending)) =
            super::imap_expunge::follow_up(&pending, &response, next_request)
                .or_else(|| super::flag_completion::follow_up(&pending, &response, next_request))
                .or_else(|| super::imap_copy::follow_up(&pending, &response, next_request))
                .or_else(|| {
                    super::pop3_protocol::follow_up(
                        &pending,
                        &response,
                        next_request,
                        maildrops,
                        slot,
                        generation,
                    )
                })
        {
            let Some(session) = sessions
                .get_mut(slot)
                .and_then(Option::as_mut)
                .map(SessionLease::get_mut)
            else {
                continue;
            };
            protocol::submit(
                session,
                request,
                next_pending,
                slot,
                generation,
                worker,
                owners,
            );
            continue;
        }
        super::imap_idle::notify_expunge(sessions, slot, &pending, &response);
        super::imap_idle::notify_flags(sessions, slot, &pending, &response);
        super::imap_idle::notify_transfer(sessions, slot, &pending, &response);
        let Some(session) = sessions
            .get_mut(slot)
            .and_then(Option::as_mut)
            .map(SessionLease::get_mut)
        else {
            continue;
        };
        if super::pop3_completion::handles(&pending, &response) {
            super::pop3_completion::write(session, pending, response, maildrops, slot, generation)?;
            protocol::process_input(session, slot, generation, worker, owners, next_request)?;
            continue;
        }
        super::completion::write_completion(session, pending, response)?;
        protocol::process_input(session, slot, generation, worker, owners, next_request)?;
    }
    Ok(unclaimed)
}
