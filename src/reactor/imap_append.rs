//! Admission и завершение принадлежащего IMAP-сессии APPEND literal.

use super::*;

/// Проверяет папку у владельца store до выдачи literal continuation.
#[allow(clippy::too_many_arguments)]
pub(super) fn begin(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    literal_size: usize,
    metadata: crate::store::AppendMetadata,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    if literal_size > MAX_MESSAGE {
        return session.push(format!("{tag} NO [TOOBIG] message exceeds limit\r\n").as_bytes());
    }
    let name = match mailbox {
        imap::MailboxName::Inbox => "INBOX".to_owned(),
        imap::MailboxName::Named(name) => name,
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::ResolveFolder {
            request_id: id,
            account,
            name,
        },
        Pending::ResolveForAppend {
            tag,
            literal_size,
            metadata,
        },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Только успешное разрешение папки переводит session в literal-receive state.
pub(super) fn resolved(
    session: &mut Session,
    tag: String,
    literal_size: usize,
    metadata: crate::store::AppendMetadata,
    result: io::Result<String>,
) -> io::Result<()> {
    match result {
        Ok(mailbox) => {
            session.imap_literal = Some(LiteralReceiver::new(tag, mailbox, literal_size, metadata));
            session.push(b"+ Ready for literal data\r\n")
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            session.push(format!("{tag} NO [TRYCREATE] mailbox does not exist\r\n").as_bytes())
        }
        Err(_) => session.push(format!("{tag} NO APPEND unavailable\r\n").as_bytes()),
    }
}

/// Делает один неблокирующий шаг literal receiver и формирует durable request.
pub(super) fn consume(
    session: &mut Session,
    request_id: &mut u64,
) -> io::Result<Option<(Request, Pending)>> {
    let progress = session
        .imap_literal
        .as_mut()
        .expect("literal receiver checked")
        .consume(&mut session.input);
    match progress {
        imap_literal::LiteralProgress::Waiting => Ok(None),
        imap_literal::LiteralProgress::InvalidTerminator(tag) => {
            session.imap_literal = None;
            session.input.clear();
            session.push(format!("{tag} BAD invalid literal terminator\r\n").as_bytes())?;
            Ok(None)
        }
        imap_literal::LiteralProgress::Complete(completed) => {
            session.imap_literal = None;
            let id = *request_id;
            *request_id = request_id.wrapping_add(1).max(1);
            Ok(Some((
                Request::Append {
                    request_id: id,
                    mailbox: completed.mailbox.clone(),
                    bytes: completed.bytes,
                    metadata: completed.metadata,
                },
                Pending::ImapAppend {
                    tag: completed.tag,
                    mailbox: completed.mailbox,
                },
            )))
        }
    }
}
