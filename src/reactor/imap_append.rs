//! Admission и завершение принадлежащего IMAP-сессии APPEND literal.

use super::*;

/// Резервирует bounded literal только после TLS и успешной аутентификации.
pub(super) fn begin(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    literal_size: usize,
    metadata: crate::store::AppendMetadata,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    if let imap::MailboxName::Named(_) = mailbox {
        return session.push(format!("{tag} NO [TRYCREATE] mailbox does not exist\r\n").as_bytes());
    }
    if literal_size > MAX_MESSAGE {
        return session.push(format!("{tag} NO message exceeds limit\r\n").as_bytes());
    }
    session.imap_literal = Some(LiteralReceiver::new(tag, account, literal_size, metadata));
    session.push(b"+ Ready for literal data\r\n")
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
