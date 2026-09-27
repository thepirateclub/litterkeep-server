//! Admission и завершение принадлежащего IMAP-сессии APPEND literal.

use super::*;

#[path = "imap_append_utf8.rs"]
mod utf8;

/// Проверяет папку у владельца store до выдачи literal continuation.
#[allow(clippy::too_many_arguments)]
pub(super) fn begin(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    literal_size: usize,
    literal_mode: crate::imap::literal::Mode,
    metadata: crate::store::AppendMetadata,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        if matches!(literal_mode, crate::imap::literal::Mode::NonSynchronizing) {
            session.push(b"* BYE non-synchronizing APPEND requires authentication\r\n")?;
            session.termination = Some(session::SessionTermination::InvalidImapLiteral);
            return Ok(());
        }
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    if let (crate::imap::literal::Mode::NonSynchronizing, true) =
        (literal_mode, literal_size > 4096)
    {
        session
            .push(b"* BYE [TOOBIG] non-synchronizing APPEND literal exceeds LITERAL- limit\r\n")?;
        session.termination = Some(session::SessionTermination::InvalidImapLiteral);
        return Ok(());
    }
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
            literal_mode,
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
    literal_mode: crate::imap::literal::Mode,
    metadata: crate::store::AppendMetadata,
    result: io::Result<String>,
) -> io::Result<()> {
    match result {
        Ok(mailbox) => {
            session.imap_literal = Some(LiteralReceiver::new(tag, mailbox, literal_size, metadata));
            match literal_mode {
                crate::imap::literal::Mode::Synchronizing => {
                    session.push(b"+ Ready for literal data\r\n")
                }
                crate::imap::literal::Mode::NonSynchronizing => Ok(()),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if matches!(literal_mode, crate::imap::literal::Mode::NonSynchronizing) {
                session.push(b"* BYE [TRYCREATE] APPEND mailbox does not exist\r\n")?;
                session.termination = Some(session::SessionTermination::InvalidImapLiteral);
                return Ok(());
            }
            session.push(format!("{tag} NO [TRYCREATE] mailbox does not exist\r\n").as_bytes())
        }
        Err(_) if matches!(literal_mode, crate::imap::literal::Mode::NonSynchronizing) => {
            session.push(b"* BYE APPEND mailbox resolution failed\r\n")?;
            session.termination = Some(session::SessionTermination::InvalidImapLiteral);
            Ok(())
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
            // RFC 3502 использует пустой literal для отмены и запрещает его публикацию.
            if completed.bytes.is_empty() {
                session.push(format!("{} NO APPEND cancelled\r\n", completed.tag).as_bytes())?;
                return Ok(None);
            }
            if matches!(
                session.imap_string_mode,
                imap::tokenizer::StringMode::Legacy
            ) && utf8::has_eight_bit_header(&completed.bytes)
            {
                session.push(
                    format!(
                        "{} NO UTF-8 headers require ENABLE UTF8=ACCEPT\r\n",
                        completed.tag
                    )
                    .as_bytes(),
                )?;
                return Ok(None);
            }
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
