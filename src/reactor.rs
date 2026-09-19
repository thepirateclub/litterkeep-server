//! One Linux `poll(2)` reactor for both public plaintext protocol listeners.
//!
//! This deliberately keeps protocol sessions as state machines. A session never
//! owns `Store`: it submits a bounded request to the single storage worker and
//! waits for a completion identified by its slot and generation.
use crate::platform::linux_poll::{self, Interest};
use crate::runtime::{Orchestrator, ResourceLease};
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
#[path = "reactor/completion_retry.rs"]
mod completion_retry;
#[path = "reactor/deadline.rs"]
mod deadline;
#[path = "reactor/flag_completion.rs"]
mod flag_completion;
#[path = "reactor/imap_append.rs"]
mod imap_append;
#[path = "reactor/imap_capability.rs"]
mod imap_capability;
#[path = "reactor/imap_check.rs"]
mod imap_check;
#[path = "reactor/imap_copy.rs"]
mod imap_copy;
#[path = "reactor/imap_expunge.rs"]
mod imap_expunge;
#[path = "reactor/imap_fetch_completion.rs"]
mod imap_fetch_completion;
#[path = "reactor/imap_fetch_response.rs"]
mod imap_fetch_response;
#[path = "reactor/imap_folders.rs"]
mod imap_folders;
#[path = "reactor/imap_idle.rs"]
mod imap_idle;
#[path = "reactor/imap_list.rs"]
mod imap_list;
#[path = "reactor/imap_literal.rs"]
mod imap_literal;
#[path = "reactor/imap_search.rs"]
mod imap_search;
#[path = "reactor/imap_select.rs"]
mod imap_select;
#[path = "reactor/imap_selected.rs"]
mod imap_selected;
#[path = "reactor/imap_starttls.rs"]
mod imap_starttls;
#[path = "reactor/imap_status.rs"]
mod imap_status;
#[path = "reactor/imap_subscriptions.rs"]
mod imap_subscriptions;
#[path = "reactor/local_delivery.rs"]
mod local_delivery;
#[path = "reactor/pending.rs"]
mod pending;
#[path = "reactor/protocol.rs"]
mod protocol;
#[path = "reactor/session.rs"]
mod session;
#[path = "reactor/session_accept.rs"]
mod session_accept;
#[path = "reactor/session_io.rs"]
mod session_io;
#[path = "reactor/smtp_completion.rs"]
mod smtp_completion;
#[path = "reactor/smtp_protocol.rs"]
mod smtp_protocol;
use completion::complete;
use completion_retry::retry_submissions;
use imap_idle::IdleState;
use imap_literal::LiteralReceiver;
use pending::Pending;
use protocol::process_input;
use session::{Session, SmtpState};
use session_io::{flush, read_session};

type SessionLease = ResourceLease<Session>;

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
enum TlsMode {
    Plain,
    Implicit(Arc<ServerConfig>),
    StartTls(Arc<ServerConfig>),
}

/// Запускает plaintext SMTP и IMAP для локального peer-теста без паролей.
pub fn serve(smtp_address: &str, imap_address: &str, root: impl AsRef<Path>) -> io::Result<()> {
    serve_with_tls(smtp_address, imap_address, root, TlsMode::Plain)
}

/// Запускает implicit-TLS SMTP submission и IMAP с одним material для listener-ов.
pub fn serve_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_with_tls(
        smtp_address,
        imap_address,
        root,
        TlsMode::Implicit(tls_config),
    )
}

/// Запускает plaintext listener с доступным SMTP STARTTLS upgrade.
pub fn serve_starttls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_config: Arc<ServerConfig>,
) -> io::Result<()> {
    serve_with_tls(
        smtp_address,
        imap_address,
        root,
        TlsMode::StartTls(tls_config),
    )
}

/// Владеет listener-ами, worker-ом и состоянием TLS каждого session.
fn serve_with_tls(
    smtp_address: &str,
    imap_address: &str,
    root: impl AsRef<Path>,
    tls_mode: TlsMode,
) -> io::Result<()> {
    let smtp_listener = TcpListener::bind(smtp_address)?;
    let imap_listener = TcpListener::bind(imap_address)?;
    smtp_listener.set_nonblocking(true)?;
    imap_listener.set_nonblocking(true)?;
    let mut worker = WakingStorageWorker::start(root, 64)?;
    let mut lifecycle: Orchestrator<Session> = Orchestrator::new();
    let mut sessions: Vec<Option<SessionLease>> = Vec::new();
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
            if let Some(lease) = session {
                let session = lease.get();
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
        deadline::expire_handshakes(&mut sessions, &mut lifecycle)?;
        for (ready, target) in ready.into_iter().zip(targets) {
            match target {
                Target::SmtpListener if ready.readable => session_accept::accept(
                    &smtp_listener,
                    Kind::Smtp,
                    &mut sessions,
                    &mut next_generation,
                    &tls_mode,
                    &mut lifecycle,
                )?,
                Target::ImapListener if ready.readable => session_accept::accept(
                    &imap_listener,
                    Kind::Imap,
                    &mut sessions,
                    &mut next_generation,
                    &tls_mode,
                    &mut lifecycle,
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
                        close_session(entry, &mut lifecycle)?;
                        continue;
                    }
                    if ready.writable {
                        if flush(entry)? {
                            close_session(entry, &mut lifecycle)?;
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
                            close_session(entry, &mut lifecycle)?;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Закрывает socket/TLS value и terminal task metadata через lifecycle owner.
fn close_session(
    entry: &mut Option<SessionLease>,
    lifecycle: &mut Orchestrator<Session>,
) -> io::Result<()> {
    let Some(lease) = entry.take() else {
        return Ok(());
    };
    lifecycle
        .close(lease)
        .map_err(|error| io::Error::other(format!("session close failed: {error:?}")))
}
