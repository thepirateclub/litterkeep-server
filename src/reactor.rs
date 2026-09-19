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
#[path = "reactor/protocol.rs"]
mod protocol;
use protocol::{complete, process_input, retry_submissions};

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
#[derive(Clone)]
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
