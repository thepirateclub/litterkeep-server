//! Главный poll-loop SMTP/IMAP; вынесен из composition root reactor-а.

use super::*;

pub(super) fn serve(
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
    let mut generation = 1_u64;
    let mut request = 1_u64;
    let mut owners = HashMap::new();
    loop {
        step(
            &smtp_listener,
            &imap_listener,
            &tls_mode,
            &mut worker,
            &mut lifecycle,
            &mut sessions,
            &mut generation,
            &mut request,
            &mut owners,
        )?;
    }
}

#[allow(clippy::too_many_arguments)]
fn step(
    smtp: &TcpListener,
    imap: &TcpListener,
    tls: &TlsMode,
    worker: &mut WakingStorageWorker,
    lifecycle: &mut Orchestrator<Session>,
    sessions: &mut Vec<Option<SessionLease>>,
    generation: &mut u64,
    request: &mut u64,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) -> io::Result<()> {
    let mut targets = vec![
        Target::SmtpListener,
        Target::ImapListener,
        Target::WorkerWake,
    ];
    let mut poll_fds = vec![
        (
            smtp.as_fd(),
            Interest {
                readable: true,
                writable: false,
            },
        ),
        (
            imap.as_fd(),
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
            let value = lease.get();
            targets.push(Target::Session(slot, value.generation));
            poll_fds.push((
                value.stream.as_fd(),
                Interest {
                    readable: value.needs_read(),
                    writable: value.needs_write(),
                },
            ));
        }
    }
    let ready = linux_poll::wait(&poll_fds, deadline::poll_timeout(sessions))?;
    deadline::expire_handshakes(sessions, lifecycle)?;
    for (ready, target) in ready.into_iter().zip(targets) {
        dispatch(
            ready, target, smtp, imap, tls, worker, lifecycle, sessions, generation, request,
            owners,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    ready: crate::platform::linux_poll::Ready,
    target: Target,
    smtp: &TcpListener,
    imap: &TcpListener,
    tls: &TlsMode,
    worker: &mut WakingStorageWorker,
    lifecycle: &mut Orchestrator<Session>,
    sessions: &mut Vec<Option<SessionLease>>,
    generation: &mut u64,
    request: &mut u64,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) -> io::Result<()> {
    match target {
        Target::SmtpListener if ready.readable => {
            session_accept::accept(smtp, Kind::Smtp, sessions, generation, tls, lifecycle)
        }
        Target::ImapListener if ready.readable => {
            session_accept::accept(imap, Kind::Imap, sessions, generation, tls, lifecycle)
        }
        Target::WorkerWake if ready.readable => {
            worker.drain_wakeup()?;
            complete(worker, sessions, owners, request)?;
            retry_submissions(worker, sessions, owners)
        }
        Target::Session(slot, generation) => service_session(
            ready, slot, generation, worker, lifecycle, sessions, owners, request,
        ),
        _ => Ok(()),
    }
}

fn service_session(
    ready: crate::platform::linux_poll::Ready,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    lifecycle: &mut Orchestrator<Session>,
    sessions: &mut [Option<SessionLease>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request: &mut u64,
) -> io::Result<()> {
    let Some(entry) = sessions.get_mut(slot) else {
        return Ok(());
    };
    if ready.closed_or_error {
        return close(entry, lifecycle);
    }
    if ready.writable && flush(entry)? {
        close(entry, lifecycle)?;
        return Ok(());
    }
    if ready.readable && read_session(entry, slot, generation, worker, owners, request)? {
        close(entry, lifecycle)?;
    }
    Ok(())
}
pub(super) fn close(
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
