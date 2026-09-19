//! One Linux `poll(2)` reactor for both public plaintext protocol listeners.
//!
//! This deliberately keeps protocol sessions as state machines. A session never
//! owns `Store`: it submits a bounded request to the single storage worker and
//! waits for a completion identified by its slot and generation.
use crate::platform::linux_poll::{self, Interest};
use crate::smtp;
use crate::storage_worker::{Request, Response, WakingStorageWorker};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsFd;
use std::path::Path;

const MAX_SESSIONS: usize = 256;
const MAX_INPUT: usize = 1024 * 1024;
const MAX_MESSAGE: usize = 10 * 1024 * 1024;
const MAX_OUTPUT: usize = MAX_MESSAGE + 16 * 1024;
const ACCEPT_BUDGET: usize = 16;
const READ_BUDGET: usize = 16 * 1024;

#[derive(Clone, Copy)]
enum Kind {
    Smtp,
    Imap,
}
#[derive(Clone, Copy)]
enum Target {
    SmtpListener,
    ImapListener,
    WorkerWake,
    Session(usize, u64),
}
#[derive(Clone, Copy)]
enum Pending {
    Append,
    ReadForSelect(String),
    ReadForFetch(usize, String),
}

struct Session {
    generation: u64,
    kind: Kind,
    stream: TcpStream,
    input: Vec<u8>,
    output: Vec<u8>,
    pending_submit: Option<(Request, Pending)>,
    pending: Option<Pending>,
    smtp: SmtpState,
}

enum SmtpState {
    Command(smtp::Transaction),
    Data(Vec<u8>),
}

impl Session {
    fn new(kind: Kind, stream: TcpStream, generation: u64) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        let mut result = Self {
            generation,
            kind,
            stream,
            input: Vec::new(),
            output: Vec::new(),
            pending_submit: None,
            pending: None,
            smtp: SmtpState::Command(smtp::Transaction::default()),
        };
        result.push(match kind {
            Kind::Smtp => b"220 litterkeep.test ESMTP litterkeep-server\r\n",
            Kind::Imap => b"* OK IMAP4rev1 litterkeep-server\r\n",
        })?;
        Ok(result)
    }
    fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.output.len().saturating_add(bytes.len()) > MAX_OUTPUT {
            return Err(io::Error::other("session output limit"));
        }
        self.output.extend_from_slice(bytes);
        Ok(())
    }
}

/// Start SMTP and IMAP in one process. Storage is opened exactly once.
pub fn serve(smtp_address: &str, imap_address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    let smtp_listener = TcpListener::bind(smtp_address)?;
    let imap_listener = TcpListener::bind(imap_address)?;
    smtp_listener.set_nonblocking(true)?;
    imap_listener.set_nonblocking(true)?;
    let mut worker = WakingStorageWorker::start(root, 64)?;
    let mut sessions: Vec<Option<Session>> = Vec::new();
    let mut next_generation = 1_u64;
    let mut next_request = 1_u64;
    let mut owners: HashMap<u64, (usize, u64, Pending)> = HashMap::new();

    loop {
        let mut targets = vec![
            Target::SmtpListener,
            Target::ImapListener,
            Target::WorkerWake,
        ];
        let mut poll_fds = vec![
            (
                smtp_listener.as_fd(),
                Interest {
                    readable: true,
                    writable: false,
                },
            ),
            (
                imap_listener.as_fd(),
                Interest {
                    readable: true,
                    writable: false,
                },
            ),
            (
                worker.wake_reader().as_fd(),
                Interest {
                    readable: true,
                    writable: false,
                },
            ),
        ];
        for (slot, session) in sessions.iter().enumerate() {
            if let Some(session) = session {
                let read = session.pending.is_none()
                    && session.pending_submit.is_none()
                    && session.output.len() < MAX_OUTPUT;
                let write = !session.output.is_empty();
                targets.push(Target::Session(slot, session.generation));
                poll_fds.push((
                    session.stream.as_fd(),
                    Interest {
                        readable: read,
                        writable: write,
                    },
                ));
            }
        }
        let ready = linux_poll::wait(&poll_fds, -1)?;
        for (ready, target) in ready.into_iter().zip(targets) {
            match target {
                Target::SmtpListener if ready.readable => accept(
                    &smtp_listener,
                    Kind::Smtp,
                    &mut sessions,
                    &mut next_generation,
                )?,
                Target::ImapListener if ready.readable => accept(
                    &imap_listener,
                    Kind::Imap,
                    &mut sessions,
                    &mut next_generation,
                )?,
                Target::WorkerWake if ready.readable => {
                    worker.drain_wakeup()?;
                    complete(&mut worker, &mut sessions, &mut owners, &mut next_request)?;
                    retry_submissions(&mut worker, &mut sessions, &mut owners)?;
                }
                Target::Session(slot, generation) => {
                    if ready.closed_or_error {
                        sessions[slot] = None;
                        continue;
                    }
                    if ready.writable && flush(&mut sessions[slot])? {
                        sessions[slot] = None;
                        continue;
                    }
                    if ready.readable
                        && read_session(
                            &mut sessions[slot],
                            slot,
                            generation,
                            &mut worker,
                            &mut owners,
                            &mut next_request,
                        )?
                    {
                        sessions[slot] = None;
                    }
                }
                _ => {}
            }
        }
    }
}

fn accept(
    listener: &TcpListener,
    kind: Kind,
    sessions: &mut Vec<Option<Session>>,
    next: &mut u64,
) -> io::Result<()> {
    for _ in 0..ACCEPT_BUDGET {
        match listener.accept() {
            Ok((stream, _)) if sessions.iter().filter(|s| s.is_some()).count() < MAX_SESSIONS => {
                let session = Session::new(kind, stream, *next)?;
                *next = next.wrapping_add(1).max(1);
                if let Some(slot) = sessions.iter().position(Option::is_none) {
                    sessions[slot] = Some(session);
                } else {
                    sessions.push(Some(session));
                }
            }
            Ok((stream, _)) => drop(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn flush(session: &mut Option<Session>) -> io::Result<bool> {
    let Some(session) = session else {
        return Ok(true);
    };
    match session.stream.write(&session.output) {
        Ok(0) => Ok(true),
        Ok(n) => {
            session.output.drain(..n);
            Ok(false)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(false),
        Err(_) => Ok(true),
    }
}

fn read_session(
    session: &mut Option<Session>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    let Some(session) = session else {
        return Ok(true);
    };
    let mut bytes = [0_u8; READ_BUDGET];
    match session.stream.read(&mut bytes) {
        Ok(0) => return Ok(true),
        Ok(count) => session.input.extend_from_slice(&bytes[..count]),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
        Err(_) => return Ok(true),
    }
    if session.input.len() > MAX_INPUT {
        return Ok(true);
    }
    process_input(session, slot, generation, worker, owners, request_id)?;
    Ok(false)
}

fn take_line(input: &mut Vec<u8>) -> Option<Vec<u8>> {
    let end = input.windows(2).position(|pair| pair == b"\r\n")?;
    Some(input.drain(..end + 2).collect())
}

fn process_input(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    while session.pending.is_none() && session.pending_submit.is_none() {
        let Some(line) = take_line(&mut session.input) else {
            break;
        };
        match session.kind {
            Kind::Smtp => smtp_line(session, line, slot, generation, worker, owners, request_id)?,
            Kind::Imap => imap_line(session, line, slot, generation, worker, owners, request_id)?,
        }
    }
    Ok(())
}

fn submit(
    session: &mut Session,
    request: Request,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) {
    match worker.worker.submit(request) {
        Ok(()) => {
            let request_id = match &request {
                Request::Append { request_id, .. } | Request::Read { request_id, .. } => {
                    *request_id
                }
            };
            owners.insert(request_id, (slot, generation, pending));
            session.pending = Some(pending);
        }
        Err(std::sync::mpsc::TrySendError::Full(request)) => {
            session.pending_submit = Some((request, pending))
        }
        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
            let _ = session.push(b"451 storage unavailable\r\n");
        }
    }
}

fn smtp_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if let SmtpState::Data(raw) = &mut session.smtp {
        if line == b".\r\n" {
            let bytes = std::mem::take(raw);
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            let id = *request_id;
            *request_id = request_id.wrapping_add(1).max(1);
            submit(
                session,
                Request::Append {
                    request_id: id,
                    mailbox: "bob".into(),
                    bytes,
                },
                Pending::Append,
                slot,
                generation,
                worker,
                owners,
            );
        } else if raw.len().saturating_add(line.len()) > MAX_MESSAGE {
            session.smtp = SmtpState::Command(smtp::Transaction::default());
            session.push(b"552 message too large\r\n")?;
        } else {
            if line.starts_with(b"..") {
                raw.extend_from_slice(&line[1..]);
            } else {
                raw.extend_from_slice(&line);
            }
        }
        return Ok(());
    }
    let text = std::str::from_utf8(&line).unwrap_or("");
    let command = match smtp::parse(text) {
        Ok(command) => command,
        Err(_) => {
            session.push(b"500 syntax error\r\n")?;
            return Ok(());
        }
    };
    if command == smtp::Command::Quit {
        session.push(b"221 bye\r\n")?;
        return Ok(());
    }
    if matches!(command, smtp::Command::Ehlo(_)) {
        session.push(b"250-litterkeep.test\r\n250 SIZE 10485760\r\n")?;
        return Ok(());
    }
    let SmtpState::Command(tx) = &mut session.smtp else {
        unreachable!()
    };
    match tx.apply(&command) {
        Ok(true) => {
            session.smtp = SmtpState::Data(Vec::new());
            session.push(b"354 end with <CRLF>.<CRLF>\r\n")?;
        }
        Ok(false) => session.push(b"250 ok\r\n")?,
        Err(_) => session.push(b"503 bad sequence\r\n")?,
    }
    Ok(())
}

fn imap_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let text = std::str::from_utf8(&line).unwrap_or("");
    let mut parts = text.split_whitespace();
    let tag = parts.next().unwrap_or("*");
    match parts.next().unwrap_or("").to_ascii_uppercase().as_str() {
        "LOGIN" => session.push(format!("{tag} OK\r\n").as_bytes())?,
        "LOGOUT" => {
            session.push(b"* BYE\r\n")?;
            session.push(format!("{tag} OK\r\n").as_bytes())?;
        }
        "SELECT" => queue_read(
            session,
            Pending::ReadForSelect(tag.into()),
            slot,
            generation,
            worker,
            owners,
            request_id,
        ),
        "FETCH" => {
            let message = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            queue_read(
                session,
                Pending::ReadForFetch(message, tag.into()),
                slot,
                generation,
                worker,
                owners,
                request_id,
            );
        }
        _ => session.push(format!("{tag} BAD\r\n").as_bytes())?,
    }
    Ok(())
}

fn queue_read(
    session: &mut Session,
    pending: Pending,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) {
    let id = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    submit(
        session,
        Request::Read {
            request_id: id,
            mailbox: "bob".into(),
        },
        pending,
        slot,
        generation,
        worker,
        owners,
    );
}

fn complete(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<Session>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    next_request: &mut u64,
) -> io::Result<()> {
    while let Ok(response) = worker.worker.try_response() {
        let id = match &response {
            Response::Appended { request_id, .. } | Response::Read { request_id, .. } => {
                *request_id
            }
        };
        let Some((slot, generation, pending)) = owners.remove(&id) else {
            continue;
        };
        let Some(session) = sessions.get_mut(slot).and_then(Option::as_mut) else {
            continue;
        };
        if session.generation != generation {
            continue;
        }
        session.pending = None;
        match (pending, response) {
            (Pending::Append, Response::Appended { result: Ok(_), .. }) => {
                session.push(b"250 queued\r\n")?
            }
            (Pending::Append, Response::Appended { result: Err(_), .. }) => {
                session.push(b"451 storage failure\r\n")?
            }
            (
                Pending::ReadForSelect(tag),
                Response::Read {
                    result: Ok(messages),
                    ..
                },
            ) => session.push(
                format!(
                    "* {} EXISTS\r\n* OK [READ-ONLY]\r\n{tag} OK\r\n",
                    messages.len(),
                )
                .as_bytes(),
            )?,
            (
                Pending::ReadForFetch(n, tag),
                Response::Read {
                    result: Ok(messages),
                    ..
                },
            ) => {
                if let Some(message) = messages.get(n.saturating_sub(1)) {
                    session.push(
                        format!("* {n} FETCH (RFC822 {{{}}}\r\n", message.bytes.len()).as_bytes(),
                    )?;
                    session.push(&message.bytes)?;
                    session.push(format!("\r\n)\r\n{tag} OK\r\n").as_bytes())?;
                } else {
                    session.push(format!("{tag} NO\r\n").as_bytes())?
                }
            }
            (_, Response::Read { .. }) => session.push(b"451 storage failure\r\n")?,
            _ => {}
        }
        process_input(session, slot, generation, worker, owners, next_request)?;
    }
    Ok(())
}

fn retry_submissions(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<Session>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) -> io::Result<()> {
    for (slot, session) in sessions.iter_mut().enumerate() {
        let Some(session) = session else { continue };
        let Some((request, pending)) = session.pending_submit.take() else {
            continue;
        };
        let generation = session.generation;
        submit(session, request, pending, slot, generation, worker, owners);
    }
    Ok(())
}
