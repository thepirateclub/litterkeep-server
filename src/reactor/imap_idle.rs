//! Event-driven IMAP IDLE state и доставка mailbox notifications.

use super::*;

/// Контекст одного активного IDLE command.
pub(super) struct IdleState {
    pub(super) tag: String,
    pub(super) mailbox: String,
}

/// Начинает ожидание только в selected state.
pub(super) fn begin(session: &mut Session, tag: String) -> io::Result<()> {
    let Some(mailbox) = session.selected_mailbox.clone() else {
        return session.push(format!("{tag} NO select mailbox first\r\n").as_bytes());
    };
    session.imap_idle = Some(IdleState { tag, mailbox });
    session.push(b"+ idling\r\n")
}

/// DONE завершает сохранённый tagged command; иные строки не являются командами.
pub(super) fn continuation(session: &mut Session, line: &[u8]) -> io::Result<()> {
    match imap::parse_idle_continuation(std::str::from_utf8(line).unwrap_or("")) {
        imap::IdleContinuation::Done => {
            let Some(idle) = session.imap_idle.take() else {
                return Ok(());
            };
            session.push(format!("{} OK IDLE completed\r\n", idle.tag).as_bytes())
        }
        imap::IdleContinuation::Invalid => session.push(b"* BAD expected DONE\r\n"),
    }
}

/// Извлекает mailbox event только из успешного durable append completion.
pub(super) fn notify_completion(
    sessions: &mut [Option<SessionLease>],
    pending: &Pending,
    response: &Response,
) -> io::Result<()> {
    let Response::Appended {
        result: Ok(uid), ..
    } = response
    else {
        return Ok(());
    };
    let mut mailboxes = match pending {
        Pending::Append { mailboxes } => mailboxes.clone(),
        Pending::ImapAppend { mailbox, .. } => vec![mailbox.clone()],
        _ => return Ok(()),
    };
    mailboxes.sort_unstable();
    mailboxes.dedup();
    let changes = mailboxes
        .into_iter()
        .map(|mailbox| (mailbox, *uid))
        .collect::<Vec<_>>();
    notify(sessions, &changes)
}

/// Увеличивает EXISTS только после подтверждённого durable append.
pub(super) fn notify(
    sessions: &mut [Option<SessionLease>],
    changed_mailboxes: &[(String, u64)],
) -> io::Result<()> {
    for entry in sessions.iter_mut() {
        let Some(lease) = entry else { continue };
        let session = lease.get_mut();
        let Some(selected) = session.selected_mailbox.as_ref() else {
            continue;
        };
        let idle_matches = session
            .imap_idle
            .as_ref()
            .is_some_and(|idle| idle.mailbox == *selected);
        let Some((_, uid)) = changed_mailboxes
            .iter()
            .find(|(mailbox, _)| mailbox == selected)
        else {
            continue;
        };
        session.selected_exists = session.selected_exists.saturating_add(1);
        session.selected_recent.insert(*uid);
        if idle_matches {
            session.push(format!("* {} EXISTS\r\n", session.selected_exists).as_bytes())?;
        }
    }
    Ok(())
}

/// Рассылает уже durable EXPUNGE другим selected IDLE-сессиям того же mailbox.
/// Инициатор получает свой ответ в completion, поэтому не должен увидеть
/// вторую копию sequence events и второй раз уменьшить EXISTS.
pub(super) fn notify_expunge(
    sessions: &mut [Option<SessionLease>],
    origin: usize,
    pending: &Pending,
    response: &Response,
) -> io::Result<()> {
    let Pending::Expunge {
        mailbox,
        sequence_numbers,
        ..
    } = pending
    else {
        return Ok(());
    };
    let Response::Expunged {
        result: Ok(removed),
        ..
    } = response
    else {
        return Ok(());
    };
    if removed.len() != sequence_numbers.len() {
        return Ok(());
    }
    for (slot, entry) in sessions.iter_mut().enumerate() {
        if slot == origin {
            continue;
        }
        let Some(lease) = entry else { continue };
        let session = lease.get_mut();
        if session.selected_mailbox.as_deref() != Some(mailbox) {
            continue;
        }
        session.selected_exists = session.selected_exists.saturating_sub(removed.len());
        if session.imap_idle.is_none() {
            continue;
        }
        for number in sequence_numbers {
            session.push(format!("* {number} EXPUNGE\r\n").as_bytes())?;
        }
    }
    Ok(())
}

/// Рассылает durable FLAGS change другим IDLE-сессиям выбранной папки.
///
/// Каждый получатель кодирует `\\Recent` из собственного session-state, а не
/// копирует FLAGS инициатора: это флаг сессии, не свойства общего message blob.
pub(super) fn notify_flags(
    sessions: &mut [Option<SessionLease>],
    origin: usize,
    pending: &Pending,
    response: &Response,
) -> io::Result<()> {
    let Pending::WriteFlags { mailbox, uids, .. } = pending else {
        return Ok(());
    };
    let Response::FlagsUpdated {
        result: Ok(snapshot),
        ..
    } = response
    else {
        return Ok(());
    };
    let changed = uids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for (slot, entry) in sessions.iter_mut().enumerate() {
        if slot == origin {
            continue;
        }
        let Some(lease) = entry else { continue };
        let session = lease.get_mut();
        if session.selected_mailbox.as_deref() != Some(mailbox) {
            continue;
        }
        if session.imap_idle.is_none() {
            continue;
        }
        for (offset, message) in snapshot.messages.iter().enumerate() {
            if !changed.contains(&message.uid) {
                continue;
            }
            let sequence = offset.saturating_add(1);
            let flags = imap::flags::encode_with_recent(
                message.flags,
                session.selected_recent.contains(&message.uid),
            );
            session.push(
                format!(
                    "* {sequence} FETCH (UID {} FLAGS ({flags}))\r\n",
                    message.uid
                )
                .as_bytes(),
            )?;
        }
    }
    Ok(())
}
