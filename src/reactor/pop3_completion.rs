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
            | (
                Pending::Pop3ScramReadInbox { .. },
                Response::FolderRead { .. }
            )
            | (Pending::Pop3ScramLookup, Response::ScramVerifier(_))
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
            Pending::Pop3ScramReadInbox { account },
            Response::FolderRead {
                result: Ok(resolved),
                ..
            },
        ) => match maildrops.acquire(account, slot, generation) {
            true => {
                session.pop3_mailbox = Some(resolved.key);
                let reply = session.pop3.authenticated(resolved.snapshot.messages);
                pop3_protocol::write_reply(session, reply)
            }
            false => {
                let reply = session.pop3.maildrop_in_use();
                pop3_protocol::write_reply(session, reply)
            }
        },
        (Pending::Pop3ScramReadInbox { .. }, Response::FolderRead { result: Err(_), .. }) => {
            session.push(b"-ERR maildrop unavailable\r\n")
        }
        (Pending::Pop3ScramLookup, Response::ScramVerifier(response)) => {
            let reply = session.pop3.scram_verifier_loaded(response.result);
            pop3_protocol::write_reply(session, reply)
        }
        (
            Pending::Pop3Authenticate { .. },
            Response::Authenticated {
                result: Ok(false), ..
            },
        ) => {
            let reply = session.pop3.authentication_rejected();
            pop3_protocol::write_reply(session, reply)
        }
        (
            Pending::Pop3Authenticate { account },
            Response::Authenticated {
                result: Ok(true), ..
            },
        ) => match maildrops.owns(&account, slot, generation) {
            true => {
                session.pop3.authentication_accepted();
                Ok(())
            }
            false => {
                let reply = session.pop3.maildrop_in_use();
                pop3_protocol::write_reply(session, reply)
            }
        },
        (Pending::Pop3Authenticate { .. }, Response::Authenticated { result: Err(_), .. }) => {
            session.push(b"-ERR authentication temporarily unavailable\r\n")
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
