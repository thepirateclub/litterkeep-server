//! Durable-семантика \Seen для IMAP BODY и RFC822 FETCH items.

use super::*;

/// Запрашивает durable \Seen до выдачи обычного BODY клиенту.
pub(super) fn follow_up(
    pending: &Pending,
    response: &Response,
    next_request: &mut u64,
) -> Option<(Request, Pending)> {
    let Pending::ReadForFetch {
        addressing,
        sequence_set,
        attributes,
        tag,
        mailbox,
        read_only,
        saved_uids,
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
    if *read_only || !sets_seen(attributes) {
        return None;
    }
    let uids = unseen_uids(*addressing, sequence_set, snapshot, saved_uids);
    if uids.is_empty() {
        return None;
    }
    let request_id = *next_request;
    *next_request = next_request.wrapping_add(1).max(1);
    Some((
        Request::UpdateFlags {
            request_id,
            mailbox: mailbox.clone(),
            uids: uids.clone(),
            mutation: crate::mail_flags::FlagMutation::Add,
            flags: crate::mail_flags::MessageFlags::from_flags([
                crate::mail_flags::SystemFlag::Seen,
            ]),
            keywords: std::collections::BTreeSet::new(),
        },
        Pending::FetchReadForSeen {
            addressing: *addressing,
            sequence_set: sequence_set.clone(),
            attributes: attributes.clone(),
            tag: tag.clone(),
            mailbox: mailbox.clone(),
            uids,
            saved_uids: saved_uids.clone(),
        },
    ))
}

fn sets_seen(attributes: &[imap::FetchAttribute]) -> bool {
    attributes.iter().any(|attribute| match attribute {
        imap::FetchAttribute::Rfc822 => true,
        imap::FetchAttribute::Body { peek, .. } => !peek,
        imap::FetchAttribute::Binary { peek, .. } => !peek,
        _ => false,
    })
}

fn unseen_uids(
    addressing: imap::Addressing,
    sequence_set: &imap::sequence_set::MessageSet,
    snapshot: &crate::store::MailboxSnapshot,
    saved_uids: &std::collections::BTreeSet<u64>,
) -> Vec<u64> {
    let maximum = match addressing {
        imap::Addressing::Sequence => snapshot.messages.len() as u64,
        imap::Addressing::Uid => snapshot
            .messages
            .last()
            .map(|message| message.uid)
            .unwrap_or(0),
    };
    snapshot
        .messages
        .iter()
        .enumerate()
        .filter_map(|(offset, message)| {
            let sequence = offset.saturating_add(1) as u64;
            let candidate = match addressing {
                imap::Addressing::Sequence => sequence,
                imap::Addressing::Uid => message.uid,
            };
            match (
                imap_fetch_completion::matches(
                    sequence_set,
                    candidate,
                    message.uid,
                    maximum,
                    saved_uids,
                ),
                message.flags.contains(crate::mail_flags::SystemFlag::Seen),
            ) {
                (true, false) => Some(message.uid),
                _ => None,
            }
        })
        .collect()
}

/// Выдаёт BODY только после успешной записи флагов в файловое хранилище.
pub(super) fn write(session: &mut Session, pending: Pending, response: Response) -> io::Result<()> {
    let Pending::FetchReadForSeen {
        addressing,
        sequence_set,
        attributes,
        tag,
        saved_uids,
        ..
    } = pending
    else {
        return Ok(());
    };
    let Response::FlagsUpdated { result, .. } = response else {
        return Ok(());
    };
    let snapshot = match result {
        Ok(snapshot) => snapshot,
        Err(_) => return session.push(format!("{tag} NO FETCH failed\r\n").as_bytes()),
    };
    imap_fetch_completion::write(
        session,
        addressing,
        sequence_set,
        attributes,
        tag,
        &saved_uids,
        snapshot.messages,
    )
}

/// Публикует durable FLAGS изменения прочим ожидающим IDLE-сессиям.
pub(super) fn notify_flags(
    sessions: &mut [Option<SessionLease>],
    origin: usize,
    pending: &Pending,
    response: &Response,
) {
    let Pending::FetchReadForSeen {
        addressing,
        mailbox,
        uids,
        tag,
        ..
    } = pending
    else {
        return;
    };
    let notification = Pending::WriteFlags {
        addressing: *addressing,
        uids: uids.clone(),
        mailbox: mailbox.clone(),
        silent: true,
        tag: tag.clone(),
    };
    super::imap_idle::notify_flags(sessions, origin, &notification, response);
}

#[cfg(test)]
#[path = "imap_fetch_seen_tests.rs"]
mod tests;
