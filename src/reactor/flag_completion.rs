//! Двухфазное завершение IMAP STORE: snapshot, затем durable mutation.

use super::*;

/// Преобразует read completion в mutation request с устойчивыми UID.
pub(super) fn follow_up(
    pending: &Pending,
    response: &Response,
    next_request: &mut u64,
) -> Option<(Request, Pending)> {
    let Pending::ReadForStore {
        addressing,
        sequence_set,
        mutation,
        flags,
        silent,
        tag,
        mailbox,
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
    let maximum = match addressing {
        imap::Addressing::Sequence => snapshot.messages.len() as u64,
        imap::Addressing::Uid => snapshot
            .messages
            .last()
            .map(|message| message.uid)
            .unwrap_or(0),
    };
    let resolved = sequence_set.resolve(maximum);
    let uids = snapshot
        .messages
        .iter()
        .enumerate()
        .filter_map(|(offset, message)| {
            let sequence = offset.saturating_add(1) as u64;
            let candidate = match addressing {
                imap::Addressing::Sequence => sequence,
                imap::Addressing::Uid => message.uid,
            };
            resolved.contains(candidate).then_some(message.uid)
        })
        .collect::<Vec<_>>();
    let request_id = *next_request;
    *next_request = next_request.wrapping_add(1).max(1);
    Some((
        Request::UpdateFlags {
            request_id,
            mailbox: mailbox.clone(),
            uids: uids.clone(),
            mutation: *mutation,
            flags: *flags,
        },
        Pending::WriteFlags {
            addressing: *addressing,
            uids,
            silent: *silent,
            tag: tag.clone(),
        },
    ))
}

/// Выдаёт изменённые FLAGS только после успешного fsync store journal.
pub(super) fn write(session: &mut Session, pending: Pending, response: Response) -> io::Result<()> {
    let Pending::WriteFlags {
        addressing,
        uids,
        silent,
        tag,
    } = pending
    else {
        return Ok(());
    };
    let Response::FlagsUpdated { result, .. } = response else {
        return Ok(());
    };
    let snapshot = match result {
        Ok(snapshot) => snapshot,
        Err(_) => return session.push(format!("{tag} NO STORE failed\r\n").as_bytes()),
    };
    if !silent {
        for (offset, message) in snapshot.messages.iter().enumerate() {
            if uids.binary_search(&message.uid).is_err() {
                continue;
            }
            let sequence = offset.saturating_add(1);
            let uid = match addressing {
                imap::Addressing::Sequence => String::new(),
                imap::Addressing::Uid => format!("UID {} ", message.uid),
            };
            let flags = imap::flags::encode_with_recent(
                message.flags,
                session.selected_recent.contains(&message.uid),
            );
            session.push(format!("* {sequence} FETCH ({uid}FLAGS ({flags}))\r\n").as_bytes())?;
        }
    }
    session.push(format!("{tag} OK STORE completed\r\n").as_bytes())
}
