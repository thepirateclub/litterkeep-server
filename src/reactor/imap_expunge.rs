//! Двухфазный IMAP EXPUNGE: snapshot sequence-map, затем durable tombstone.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn queue(
    session: &mut Session,
    tag: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
    close: bool,
    uid_set: Option<imap::sequence_set::SequenceSet>,
) -> io::Result<()> {
    if matches!((session.selected_read_only, close), (true, false)) {
        return session.push(format!("{tag} NO [READ-ONLY] EXPUNGE denied\r\n").as_bytes());
    }
    let Some(mailbox) = session.selected_mailbox.clone() else {
        return session.push(format!("{tag} NO select mailbox first\r\n").as_bytes());
    };
    if session.selected_read_only {
        return close_completed(session, tag);
    }
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: mailbox.clone(),
        },
        Pending::ReadForExpunge {
            tag,
            mailbox,
            close,
            uid_set,
        },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

pub(super) fn follow_up(
    pending: &Pending,
    response: &Response,
    next_request: &mut u64,
) -> Option<(Request, Pending)> {
    let Pending::ReadForExpunge {
        tag,
        mailbox,
        close,
        uid_set,
    } = pending
    else {
        return None;
    };
    let Response::Read {
        result: Ok(snapshot),
        ..
    } = response
    else {
        return None;
    };
    let mut uids = Vec::new();
    let mut sequence_numbers = Vec::new();
    let mut removed = 0_usize;
    let resolved_uids = uid_set.as_ref().map(|set| {
        set.resolve(
            snapshot
                .messages
                .last()
                .map(|message| message.uid)
                .unwrap_or(0),
        )
    });
    for (offset, message) in snapshot.messages.iter().enumerate() {
        if !message
            .flags
            .contains(crate::mail_flags::SystemFlag::Deleted)
        {
            continue;
        }
        if resolved_uids
            .as_ref()
            .is_some_and(|set| !set.contains(message.uid))
        {
            continue;
        }
        let sequence = offset.saturating_add(1).saturating_sub(removed);
        uids.push(message.uid);
        sequence_numbers.push(sequence);
        removed = removed.saturating_add(1);
    }
    let request_id = *next_request;
    *next_request = next_request.wrapping_add(1).max(1);
    Some((
        Request::Expunge {
            request_id,
            mailbox: mailbox.clone(),
            uids,
        },
        Pending::Expunge {
            tag: tag.clone(),
            mailbox: mailbox.clone(),
            sequence_numbers,
            close: *close,
        },
    ))
}

pub(super) fn completed(
    session: &mut Session,
    tag: String,
    sequence_numbers: Vec<usize>,
    result: io::Result<Vec<u64>>,
    close: bool,
) -> io::Result<()> {
    let removed = match result {
        Ok(removed) if removed.len() == sequence_numbers.len() => removed,
        _ => return session.push(format!("{tag} NO EXPUNGE failed\r\n").as_bytes()),
    };
    if !close {
        for sequence in sequence_numbers {
            session.push(format!("* {sequence} EXPUNGE\r\n").as_bytes())?;
        }
    }
    session.selected_exists = session.selected_exists.saturating_sub(removed.len());
    match close {
        true => close_completed(session, tag),
        false => session.push(format!("{tag} OK EXPUNGE completed\r\n").as_bytes()),
    }
}

/// Завершает CLOSE только после durable удаления и забывает выбранный mailbox.
fn close_completed(session: &mut Session, tag: String) -> io::Result<()> {
    session.selected_mailbox = None;
    session.selected_exists = 0;
    session.selected_read_only = false;
    session.push(format!("{tag} OK CLOSE completed\r\n").as_bytes())
}
