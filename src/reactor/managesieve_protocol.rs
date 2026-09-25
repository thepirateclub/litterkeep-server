//! Преобразует ManageSieve replies в bounded запросы единственного Store owner.

use super::*;

pub(super) fn line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    match managesieve_literal::start(&line) {
        managesieve_literal::Start::NotLiteral => {}
        managesieve_literal::Start::Invalid => return session.push(b"NO \"invalid literal\"\r\n"),
        managesieve_literal::Start::Begin {
            receiver,
            continuation,
        } => {
            session.managesieve_literal = Some(receiver);
            if continuation {
                session.push(b"OK \"ready for literal data\"\r\n")?;
            }
            return Ok(());
        }
    }
    let command = managesieve::parse(std::str::from_utf8(&line).unwrap_or(""));
    let reply = session
        .managesieve
        .apply(command, session.is_encrypted(), session.can_starttls());
    apply(session, reply, slot, generation, worker, owners, request_id)
}

/// Поглощает уже объявленное тело script literal и сохраняет pipeline bytes.
pub(super) fn literal(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    let progress = session
        .managesieve_literal
        .as_mut()
        .expect("ManageSieve literal receiver checked")
        .consume(&mut session.input);
    match progress {
        managesieve_literal::Progress::Waiting => Ok(false),
        managesieve_literal::Progress::InvalidUtf8 => {
            session.managesieve_literal = None;
            session.push(b"NO \"script must be UTF-8\"\r\n")?;
            Ok(true)
        }
        managesieve_literal::Progress::Complete(command) => {
            session.managesieve_literal = None;
            let reply =
                session
                    .managesieve
                    .apply(command, session.is_encrypted(), session.can_starttls());
            apply(session, reply, slot, generation, worker, owners, request_id)?;
            Ok(true)
        }
    }
}

fn apply(
    session: &mut Session,
    reply: managesieve::Reply,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    match reply {
        managesieve::Reply::Text(bytes) => session.push(&bytes),
        managesieve::Reply::StartTls => {
            session.push(b"OK \"begin TLS\"\r\n")?;
            session.request_starttls();
            Ok(())
        }
        managesieve::Reply::Authenticate { response } => authenticate(
            session, response, slot, generation, worker, owners, request_id,
        ),
        reply => submit_command(session, reply, slot, generation, worker, owners, request_id),
    }
}

fn authenticate(
    session: &mut Session,
    response: String,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let credentials = match crate::sasl::parse_plain(&response) {
        Ok(value) => value,
        Err(_) => return session.push(b"NO \"invalid SASL PLAIN\"\r\n"),
    };
    let id = next(request_id);
    let account = match crate::local_route::local_login(&credentials.account) {
        Some(account) => account,
        None => return session.push(b"NO \"authentication failed\"\r\n"),
    };
    protocol::submit(
        session,
        Request::Authenticate {
            request_id: id,
            username: account.clone(),
            password: credentials.password,
        },
        Pending::ManageSieveAuthenticate { account },
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

fn submit_command(
    session: &mut Session,
    reply: managesieve::Reply,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(account) = session.authenticated_account.clone() else {
        return session.push(b"NO \"authentication required\"\r\n");
    };
    let id = next(request_id);
    let request = match reply {
        managesieve::Reply::HaveSpace { name, size } => Request::SieveHaveSpace {
            request_id: id,
            account,
            name,
            size,
        },
        managesieve::Reply::Put { name, source } => Request::SievePut {
            request_id: id,
            account,
            name,
            source,
        },
        managesieve::Reply::List => Request::SieveList {
            request_id: id,
            account,
        },
        managesieve::Reply::SetActive(name) => Request::SieveSetActive {
            request_id: id,
            account,
            name,
        },
        managesieve::Reply::Get(name) => Request::SieveGet {
            request_id: id,
            account,
            name,
        },
        managesieve::Reply::Delete(name) => Request::SieveDelete {
            request_id: id,
            account,
            name,
        },
        managesieve::Reply::Rename { old, new } => Request::SieveRename {
            request_id: id,
            account,
            old,
            new,
        },
        managesieve::Reply::Check(source) => Request::SieveCheck {
            request_id: id,
            source,
        },
        _ => return session.push(b"NO \"invalid command\"\r\n"),
    };
    protocol::submit(
        session,
        request,
        Pending::ManageSieveCommand,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

fn next(request_id: &mut u64) -> u64 {
    let value = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    value
}
