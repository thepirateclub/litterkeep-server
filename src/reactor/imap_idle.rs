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
    if !matches!(response, Response::Appended { result: Ok(_), .. }) {
        return Ok(());
    }
    let mut mailboxes = match pending {
        Pending::Append { mailboxes } => mailboxes.clone(),
        Pending::ImapAppend { mailbox, .. } => vec![mailbox.clone()],
        _ => return Ok(()),
    };
    mailboxes.sort_unstable();
    mailboxes.dedup();
    notify(sessions, &mailboxes)
}

/// Увеличивает EXISTS только после подтверждённого durable append.
pub(super) fn notify(
    sessions: &mut [Option<SessionLease>],
    changed_mailboxes: &[String],
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
        if !changed_mailboxes.iter().any(|mailbox| mailbox == selected) {
            continue;
        }
        session.selected_exists = session.selected_exists.saturating_add(1);
        if idle_matches {
            session.push(format!("* {} EXISTS\r\n", session.selected_exists).as_bytes())?;
        }
    }
    Ok(())
}
