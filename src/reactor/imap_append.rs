//! Admission и асинхронная публикация полностью собранной IMAP APPEND-команды.

use super::*;

#[path = "imap_append_utf8.rs"]
mod utf8;

/// Проверяет batch до передачи единственному Store owner-у.
#[allow(clippy::too_many_arguments)]
pub(super) fn begin(
    session: &mut Session,
    tag: String,
    mailbox: imap::MailboxName,
    messages: Vec<imap::append::AppendMessage>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(format!("{tag} NO authenticate first\r\n").as_bytes());
    };
    if messages.is_empty() {
        return session.push(format!("{tag} BAD APPEND requires a message\r\n").as_bytes());
    }
    if messages.iter().any(|message| message.bytes.is_empty()) {
        return session.push(format!("{tag} NO APPEND cancelled\r\n").as_bytes());
    }
    if utf8_headers_are_forbidden(session, &messages) {
        return session
            .push(format!("{tag} NO UTF-8 headers require ENABLE UTF8=ACCEPT\r\n").as_bytes());
    }
    let name = mailbox_name(mailbox);
    let stored_messages = messages
        .into_iter()
        .map(|message| {
            let metadata = crate::store::AppendMetadata {
                flags: message.flags,
                keywords: message.keywords,
                internal_date: message
                    .internal_date
                    .unwrap_or_else(crate::mail_date::MailDate::now),
            };
            (message.bytes, metadata)
        })
        .collect();
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::ImapAppend {
            request_id: id,
            account,
            mailbox: name.clone(),
            messages: stored_messages,
        },
        Pending::ImapAppend { tag },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

fn mailbox_name(mailbox: imap::MailboxName) -> String {
    match mailbox {
        imap::MailboxName::Inbox => "INBOX".to_owned(),
        imap::MailboxName::Named(name) => name,
    }
}

fn utf8_headers_are_forbidden(session: &Session, messages: &[imap::append::AppendMessage]) -> bool {
    let legacy = matches!(
        session.imap_string_mode,
        imap::tokenizer::StringMode::Legacy
    );
    match legacy {
        true => messages
            .iter()
            .any(|message| utf8::has_eight_bit_header(&message.bytes)),
        false => false,
    }
}
