//! One Linux `poll(2)` reactor for both public plaintext protocol listeners.
//!
//! This deliberately keeps protocol sessions as state machines. A session never
//! owns `Store`: it submits a bounded request to the single storage worker and
//! waits for a completion identified by its slot and generation.
use crate::platform::linux_poll::{self, Interest};
use crate::storage_worker::{Request, Response, WakingStorageWorker};
use crate::{imap, smtp, tls_transport::ServerTls};
use rustls::ServerConfig;
use std::collections::HashMap;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::os::fd::AsFd;
use std::{path::Path, sync::Arc};
#[path = "reactor/completion.rs"]
mod completion;
#[path = "reactor/deadline.rs"]
mod deadline;
#[path = "reactor/local_delivery.rs"]
mod local_delivery;
#[path = "reactor/protocol.rs"]
mod protocol;
#[path = "reactor/session.rs"]
mod session;
#[path = "reactor/session_io.rs"]
mod session_io;
#[path = "reactor/smtp_protocol.rs"]
mod smtp_protocol;
use completion::{complete, retry_submissions};
use protocol::process_input;
use session::{Session, SmtpState};
use session_io::{flush, read_session};

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
    Authenticate { tag: String, account: String },
    SmtpAuthenticate { account: String },
    ValidateRecipient { address: String },
    Append,
    ReadForSelect(String),
    ReadForFetch(usize, String),
}

/// Запускает plaintext SMTP и IMAP для локального peer-теста без паролей.
pub fn serve(smtp_address: &str, imap_address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    serve_with_tls(smtp_address, imap_address, root, None)
}

/// Запускает implicit-TLS SMTP submission и IMAP с одним material для listener-ов.
pub fn serve_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_with_tls(smtp_address, imap_address, root, Some(tls_config))
}

/// Владеет listener-ами, worker-ом и состоянием TLS каждого session.
fn serve_with_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Option<Arc<ServerConfig>>,
) -> io::Result<()> {
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
                let read = session.needs_read();
                let write = session.needs_write();
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
        let timeout_ms = deadline::poll_timeout(&sessions);
        let ready = linux_poll::wait(&poll_fds, timeout_ms)?;
        deadline::expire_handshakes(&mut sessions);
        for (ready, target) in ready.into_iter().zip(targets) {
            match target {
                Target::SmtpListener if ready.readable => accept(
                    &smtp_listener,
                    Kind::Smtp,
                    &mut sessions,
                    &mut next_generation,
                    tls_config.as_ref(),
                )?,
                Target::ImapListener if ready.readable => accept(
                    &imap_listener,
                    Kind::Imap,
                    &mut sessions,
                    &mut next_generation,
                    tls_config.as_ref(),
                )?,
                Target::WorkerWake if ready.readable => {
                    worker.drain_wakeup()?;
                    complete(&mut worker, &mut sessions, &mut owners, &mut next_request)?;
                    retry_submissions(&mut worker, &mut sessions, &mut owners)?;
                }
                Target::Session(slot, generation) => {
                    let Some(entry) = sessions.get_mut(slot) else {
                        continue;
                    };
                    if ready.closed_or_error {
                        *entry = None;
                        continue;
                    }
                    if ready.writable {
                        if flush(entry)? {
                            *entry = None;
                            continue;
                        }
                    }
                    if ready.readable {
                        if read_session(
                            entry,
                            slot,
                            generation,
                            &mut worker,
                            &mut owners,
                            &mut next_request,
                        )? {
                            *entry = None;
                        }
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
    tls_config: Option<&Arc<ServerConfig>>,
) -> io::Result<()> {
    for _ in 0..ACCEPT_BUDGET {
        match listener.accept() {
            Ok((stream, _)) if sessions.iter().filter(|s| s.is_some()).count() < MAX_SESSIONS => {
                let session = Session::new(kind, stream, *next, tls_config.cloned())?;
                *next = next.wrapping_add(1).max(1);
                if let Some(slot) = sessions.iter().position(Option::is_none) {
                    let Some(entry) = sessions.get_mut(slot) else {
                        return Err(io::Error::other("missing reusable session slot"));
                    };
                    *entry = Some(session);
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
