//! POP3-specific completion: worker остаётся владельцем Store до durable ответа.

use super::*;

/// Распознаёт POP3 completion без переноса чужих protocol вариантов.
pub(super) fn handles(pending: &Pending, response: &Response) -> bool {
    matches!(
        (pending, response),
        (
            Pending::Pop3Authenticate { .. },
            Response::Authenticated { .. }
        ) | (Pending::Pop3ReadInbox, Response::FolderRead { .. })
            | (Pending::Pop3Commit, Response::Expunged { .. })
    )
}

/// Превращает POP3 completion в wire result после владения Store worker-ом.
pub(super) fn write(
    session: &mut Session,
    pending: Pending,
    response: Response,
    maildrops: &mut MaildropLeases,
    slot: usize,
    generation: u64,
) -> io::Result<()> {
    match (pending, response) {
        (
            Pending::Pop3Authenticate { .. },
            Response::Authenticated {
                accepted: false, ..
            },
        ) => session.push(b"-ERR authentication failed\r\n"),
        (Pending::Pop3Authenticate { account }, Response::Authenticated { accepted: true, .. }) => {
            match maildrops.owns(&account, slot, generation) {
                true => Ok(()),
                false => pop3_protocol::write_reply(
                    session,
                    pop3::Reply::ErrorCode(pop3::ResponseCode::InUse, "maildrop is locked"),
                ),
            }
        }
        (
            Pending::Pop3ReadInbox,
            Response::FolderRead {
                result: Ok(resolved),
                ..
            },
        ) => {
            session.pop3_mailbox = Some(resolved.key);
            let reply = session.pop3.authenticated(resolved.snapshot.messages);
            pop3_protocol::write_reply(session, reply)
        }
        (Pending::Pop3ReadInbox, Response::FolderRead { result: Err(_), .. }) => {
            maildrops.release(slot, generation);
            session.push(b"-ERR maildrop unavailable\r\n")
        }
        (Pending::Pop3Commit, Response::Expunged { result: Ok(_), .. }) => {
            maildrops.release(slot, generation);
            let reply = session.pop3.commit_deletes(true);
            pop3_protocol::write_reply(session, reply)
        }
        (Pending::Pop3Commit, Response::Expunged { result: Err(_), .. }) => {
            let reply = session.pop3.commit_deletes(false);
            pop3_protocol::write_reply(session, reply)
        }
        _ => Ok(()),
    }
}
