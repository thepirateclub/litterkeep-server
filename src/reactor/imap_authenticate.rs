//! Пошаговая IMAP AUTHENTICATE FSM для собственного SASL PLAIN decoder-а.

use super::super::*;

/// Незавершённый SASL exchange принадлежит ровно одной IMAP-сессии.
pub(in crate::reactor) struct State {
    tag: String,
    mechanism: imap::AuthenticationMechanism,
}

/// Начинает exchange либо сразу обрабатывает SASL initial response.
#[allow(clippy::too_many_arguments)]
pub(super) fn begin(
    session: &mut Session,
    tag: String,
    mechanism: imap::AuthenticationMechanism,
    initial_response: Option<String>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if session.authenticated_account.is_some() {
        return session.push(format!("{tag} BAD already authenticated\r\n").as_bytes());
    }
    if !session.is_encrypted() {
        return session
            .push(format!("{tag} NO [PRIVACYREQUIRED] STARTTLS required\r\n").as_bytes());
    }
    match initial_response {
        Some(response) => queue(
            session, tag, mechanism, &response, slot, generation, worker, owners, request_id,
        ),
        None => {
            session.imap_authenticate = Some(State { tag, mechanism });
            session.push(b"+ \r\n")
        }
    }
}

/// LOGIN использует тот же worker и completion, но не SASL framing.
#[allow(clippy::too_many_arguments)]
pub(super) fn login(
    session: &mut Session,
    tag: String,
    account: String,
    password: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if session.authenticated_account.is_some() {
        return session.push(format!("{tag} BAD already authenticated\r\n").as_bytes());
    }
    if !session.is_encrypted() {
        return session
            .push(format!("{tag} NO [PRIVACYREQUIRED] STARTTLS required\r\n").as_bytes());
    }
    submit_credentials(
        session,
        tag,
        account,
        password,
        AuthenticationCommand::Login,
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

/// Принимает ровно одну continuation-строку либо явную отмену клиента.
#[allow(clippy::too_many_arguments)]
pub(super) fn continuation(
    session: &mut Session,
    line: &[u8],
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let Some(state) = session.imap_authenticate.take() else {
        return Ok(());
    };
    let response = match parse_continuation(line) {
        Continuation::Response(response) => response,
        Continuation::Cancelled => {
            return session
                .push(format!("{} BAD AUTHENTICATE cancelled\r\n", state.tag).as_bytes());
        }
        Continuation::Invalid => {
            return session.push(format!("{} BAD invalid SASL response\r\n", state.tag).as_bytes());
        }
    };
    queue(
        session,
        state.tag,
        state.mechanism,
        response,
        slot,
        generation,
        worker,
        owners,
        request_id,
    )
}

enum Continuation<'a> {
    Response(&'a str),
    Cancelled,
    Invalid,
}

fn parse_continuation(line: &[u8]) -> Continuation<'_> {
    let payload = line.strip_suffix(b"\r\n").unwrap_or(line);
    match (payload, std::str::from_utf8(payload)) {
        (b"*", _) => Continuation::Cancelled,
        (_, Ok(response)) => Continuation::Response(response),
        (_, Err(_)) => Continuation::Invalid,
    }
}

#[allow(clippy::too_many_arguments)]
fn queue(
    session: &mut Session,
    tag: String,
    mechanism: imap::AuthenticationMechanism,
    response: &str,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let credentials = match mechanism {
        imap::AuthenticationMechanism::Plain => crate::sasl::parse_plain(response),
    };
    let credentials = match credentials {
        Ok(credentials) => credentials,
        Err(_) => return session.push(format!("{tag} NO authentication failed\r\n").as_bytes()),
    };
    submit_credentials(
        session,
        tag,
        credentials.account,
        credentials.password,
        AuthenticationCommand::Authenticate,
        slot,
        generation,
        worker,
        owners,
        request_id,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn submit_credentials(
    session: &mut Session,
    tag: String,
    account: String,
    password: Vec<u8>,
    command: AuthenticationCommand,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) {
    let account = match crate::local_route::local_login(&account) {
        Some(account) => account,
        None => {
            let _ = session.push(format!("{tag} NO authentication failed\r\n").as_bytes());
            return;
        }
    };
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    protocol::submit(
        session,
        Request::Authenticate {
            request_id: id,
            username: account.clone(),
            password,
        },
        Pending::Authenticate {
            tag,
            account,
            command,
        },
        slot,
        generation,
        worker,
        owners,
    );
}

#[cfg(test)]
#[path = "imap_authenticate_tests.rs"]
mod tests;
