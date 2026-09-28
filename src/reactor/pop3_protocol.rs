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
    let exporter = session
        .tls
        .as_ref()
        .and_then(ServerTls::tls_exporter_channel_binding);
    session.pop3.update_tls_exporter(exporter);
    if session.pop3.awaiting_sasl_response() {
        let reply = match std::str::from_utf8(&line)
            .ok()
            .and_then(|wire| wire.strip_suffix("\r\n"))
        {
            Some(response) => session.pop3.sasl_plain_response(response),
            None => session.pop3.sasl_plain_response(""),
        };
        return dispatch_reply(session, reply, slot, generation, worker, owners, request_id);
    }
    if line.len() > pop3::MAX_COMMAND_LINE_SIZE {
        return session.push(b"-ERR command line too long\r\n");
    }
    let command = pop3::parse(std::str::from_utf8(&line).unwrap_or_default());
    if matches!(
        (&command, session.can_starttls()),
        (pop3::Command::Pass(_), true)
    ) {
        return session.push(b"-ERR STLS required before PASS\r\n");
    }
    let reply = session.pop3.apply(command);
    dispatch_reply(session, reply, slot, generation, worker, owners, request_id)
}

/// Направляет typed reply к auth/store/TLS action либо bounded serializer-у.
#[allow(clippy::too_many_arguments)]
fn dispatch_reply(
    session: &mut Session,
    reply: pop3::Reply,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    match reply {
        pop3::Reply::ScramVerifierLookup { account } => pop3_scram_protocol::lookup(
            session, account, slot, generation, worker, owners, request_id,
        ),
        pop3::Reply::ScramSha256VerifierLookup { account } => pop3_scram_protocol::lookup_sha256(
            session, account, slot, generation, worker, owners, request_id,
        ),
        pop3::Reply::ScramAccepted { account } => pop3_scram_protocol::authenticated(
            session, account, slot, generation, worker, owners, request_id,
        ),
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
    maildrops: &mut MaildropLeases,
    slot: usize,
    generation: u64,
) -> Option<(Request, Pending)> {
    let (
        Pending::Pop3Authenticate { account },
        Response::Authenticated {
            result: Ok(true), ..
        },
    ) = (pending, response)
    else {
        return None;
    };
    if !maildrops.acquire(account.clone(), slot, generation) {
        return None;
    }
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
    let bytes = pop3::reply_wire::encode_reply(reply)
        .ok_or_else(|| io::Error::other("POP3 reply requires reactor action"))?;
    session.push(&bytes)?;
    if session.pop3.is_update() {
        session.termination = Some(session::SessionTermination::Pop3Quit);
    }
    Ok(())
}

/// Отклоняет ограниченный по памяти oversized SASL response и восстанавливает USER.
pub(super) fn reject_oversized_sasl_response(session: &mut Session) -> io::Result<()> {
    let reply = session.pop3.authentication_rejected();
    write_reply(session, reply)
}

#[allow(clippy::too_many_arguments)]
fn authenticate(
    session: &mut Session,
    username: String,
    password: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let username = match crate::local_route::local_login(&username) {
        Some(account) => account,
        None => {
            let reply = session.pop3.authentication_rejected();
            return write_reply(session, reply);
        }
    };
    let id = next_id(request_id);
    protocol::submit(
        session,
        Request::Authenticate {
            request_id: id,
            username: username.clone(),
            password,
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
    session.pop3.reset_authorization();
    session.request_starttls();
    Ok(())
}

/// Выдаёт ненулевые request id без прямого обращения к массивам или очередям.
pub(super) fn next_id(request_id: &mut u64) -> u64 {
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    id
}
