//! POP3 wire boundary: parser, TLS policy и запросы единственному Store owner.

use super::*;

/// Обрабатывает одну POP3 command line, не позволяя password до STLS.
#[allow(clippy::too_many_arguments)]
pub(super) fn line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let command = pop3::parse(std::str::from_utf8(&line).unwrap_or_default());
    if matches!(command, pop3::Command::Pass(_)) && session.can_starttls() {
        return session.push(b"-ERR STLS required before PASS\r\n");
    }
    match session.pop3.apply(command) {
        pop3::Reply::Authenticate { username, password } => authenticate(
            session, username, password, slot, generation, worker, owners, request_id,
        ),
        pop3::Reply::CommitDeletes(uids) => {
            commit(session, uids, slot, generation, worker, owners, request_id)
        }
        pop3::Reply::StartTls => begin_tls(session),
        reply => write_reply(session, reply),
    }
}

/// Строит второй request только после успешной проверки password.
pub(super) fn follow_up(
    pending: &Pending,
    response: &Response,
    request_id: &mut u64,
) -> Option<(Request, Pending)> {
    let (Pending::Pop3Authenticate { account }, Response::Authenticated { accepted: true, .. }) =
        (pending, response)
    else {
        return None;
    };
    let id = next_id(request_id);
    Some((
        Request::ReadFolder {
            request_id: id,
            account: account.clone(),
            name: "INBOX".into(),
        },
        Pending::Pop3ReadInbox,
    ))
}

/// Кодирует все POP3 replies в одном месте, включая already-multiline frames.
pub(super) fn write_reply(session: &mut Session, reply: pop3::Reply) -> io::Result<()> {
    match reply {
        pop3::Reply::Ok(text) => session.push(format!("+OK {text}\r\n").as_bytes()),
        pop3::Reply::Error(text) => session.push(format!("-ERR {text}\r\n").as_bytes()),
        pop3::Reply::Multiline(bytes) => session.push(&bytes),
        pop3::Reply::Authenticate { .. }
        | pop3::Reply::CommitDeletes(_)
        | pop3::Reply::StartTls => Err(io::Error::other("POP3 reply requires reactor action")),
    }
}

#[allow(clippy::too_many_arguments)]
fn authenticate(
    session: &mut Session,
    username: String,
    password: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let id = next_id(request_id);
    protocol::submit(
        session,
        Request::Authenticate {
            request_id: id,
            username: username.clone(),
            password: password.into_bytes(),
        },
        Pending::Pop3Authenticate { account: username },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn commit(
    session: &mut Session,
    uids: Vec<u64>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if uids.is_empty() {
        let reply = session.pop3.commit_deletes(true);
        return write_reply(session, reply);
    }
    let Some(mailbox) = session.pop3_mailbox.clone() else {
        return session.push(b"-ERR maildrop unavailable\r\n");
    };
    let id = next_id(request_id);
    protocol::submit(
        session,
        Request::Expunge {
            request_id: id,
            mailbox,
            uids,
        },
        Pending::Pop3Commit,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// STLS меняет transport только после полной отправки +OK boundary.
fn begin_tls(session: &mut Session) -> io::Result<()> {
    if !session.can_starttls() {
        return session.push(b"-ERR STLS unavailable\r\n");
    }
    session.push(b"+OK Begin TLS negotiation\r\n")?;
    session.request_starttls();
    Ok(())
}

/// Выдаёт ненулевые request id без прямого обращения к массивам или очередям.
fn next_id(request_id: &mut u64) -> u64 {
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    id
}
