//! Главный poll-loop SMTP/IMAP; вынесен из composition root reactor-а.

use super::*;

pub(super) fn serve(
    smtp_address: Option<&str>,
    imap_address: Option<&str>,
    pop3_address: Option<&str>,
    managesieve_address: Option<&str>,
    root: impl AsRef<Path>,
    tls_mode: TlsMode,
) -> io::Result<()> {
    let smtp_listener = bind(smtp_address)?;
    let imap_listener = bind(imap_address)?;
    let pop3_listener = bind(pop3_address)?;
    let managesieve_listener = bind(managesieve_address)?;
    let mut worker = WakingStorageWorker::start(root, 64)?;
    let mut lifecycle: Orchestrator<Session> = Orchestrator::new();
    let mut sessions: Vec<Option<SessionLease>> = Vec::new();
    let mut generation = 1_u64;
    let mut request = 1_u64;
    let mut owners = HashMap::new();
    let mut maildrops = MaildropLeases::default();
    loop {
        step(
            smtp_listener.as_ref(),
            imap_listener.as_ref(),
            pop3_listener.as_ref(),
            managesieve_listener.as_ref(),
            &tls_mode,
            &mut worker,
            &mut lifecycle,
            &mut sessions,
            &mut generation,
            &mut request,
            &mut owners,
            &mut maildrops,
        )?;
    }
}

/// Создаёт только явно запрошенный listener, сохраняя ownership в poll-loop.
fn bind(address: Option<&str>) -> io::Result<Option<TcpListener>> {
    let Some(address) = address else {
        return Ok(None);
    };
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    Ok(Some(listener))
}

#[allow(clippy::too_many_arguments)]
fn step(
    smtp: Option<&TcpListener>,
    imap: Option<&TcpListener>,
    pop3: Option<&TcpListener>,
    managesieve: Option<&TcpListener>,
    tls: &TlsMode,
    worker: &mut WakingStorageWorker,
    lifecycle: &mut Orchestrator<Session>,
    sessions: &mut Vec<Option<SessionLease>>,
    generation: &mut u64,
    request: &mut u64,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    let mut targets = vec![Target::WorkerWake];
    let mut poll_fds = vec![(
        worker.wake_reader().as_fd(),
        Interest {
            readable: true,
            writable: false,
        },
    )];
    append_listener(smtp, Target::SmtpListener, &mut targets, &mut poll_fds);
    append_listener(imap, Target::ImapListener, &mut targets, &mut poll_fds);
    append_listener(pop3, Target::Pop3Listener, &mut targets, &mut poll_fds);
    append_listener(
        managesieve,
        Target::ManageSieveListener,
        &mut targets,
        &mut poll_fds,
    );
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
    deadline::expire_handshakes(sessions, lifecycle, maildrops)?;
    for (ready, target) in ready.into_iter().zip(targets) {
        dispatch(
            ready,
            target,
            smtp,
            imap,
            pop3,
            managesieve,
            tls,
            worker,
            lifecycle,
            sessions,
            generation,
            request,
            owners,
            maildrops,
        )?;
    }
    Ok(())
}

/// Добавляет listener в тот же список readiness, не создавая отдельный loop.
fn append_listener<'a>(
    listener: Option<&'a TcpListener>,
    target: Target,
    targets: &mut Vec<Target>,
    poll_fds: &mut Vec<(std::os::fd::BorrowedFd<'a>, Interest)>,
) {
    let Some(listener) = listener else {
        return;
    };
    targets.push(target);
    poll_fds.push((
        listener.as_fd(),
        Interest {
            readable: true,
            writable: false,
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    ready: crate::platform::linux_poll::Ready,
    target: Target,
    smtp: Option<&TcpListener>,
    imap: Option<&TcpListener>,
    pop3: Option<&TcpListener>,
    managesieve: Option<&TcpListener>,
    tls: &TlsMode,
    worker: &mut WakingStorageWorker,
    lifecycle: &mut Orchestrator<Session>,
    sessions: &mut Vec<Option<SessionLease>>,
    generation: &mut u64,
    request: &mut u64,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    match target {
        Target::SmtpListener if ready.readable => {
            accept(smtp, Kind::Smtp, sessions, generation, tls, lifecycle)
        }
        Target::ImapListener if ready.readable => {
            accept(imap, Kind::Imap, sessions, generation, tls, lifecycle)
        }
        Target::Pop3Listener if ready.readable => {
            accept(pop3, Kind::Pop3, sessions, generation, tls, lifecycle)
        }
        Target::ManageSieveListener if ready.readable => accept(
            managesieve,
            Kind::ManageSieve,
            sessions,
            generation,
            tls,
            lifecycle,
        ),
        Target::WorkerWake if ready.readable => {
            worker.drain_wakeup()?;
            complete(worker, sessions, owners, request, maildrops)?;
            retry_submissions(worker, sessions, owners)
        }
        Target::Session(slot, generation) => service_session(
            ready, slot, generation, worker, lifecycle, sessions, owners, request, maildrops,
        ),
        _ => Ok(()),
    }
}

/// Невозможный target не обращается к отсутствующему listener-у по индексу.
fn accept(
    listener: Option<&TcpListener>,
    kind: Kind,
    sessions: &mut Vec<Option<SessionLease>>,
    generation: &mut u64,
    tls: &TlsMode,
    lifecycle: &mut Orchestrator<Session>,
) -> io::Result<()> {
    let Some(listener) = listener else {
        return Ok(());
    };
    session_accept::accept(listener, kind, sessions, generation, tls, lifecycle)
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
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    let Some(entry) = sessions.get_mut(slot) else {
        return Ok(());
    };
    if ready.closed_or_error {
        return close(entry, lifecycle, maildrops);
    }
    if ready.writable && flush(entry)? {
        close(entry, lifecycle, maildrops)?;
        return Ok(());
    }
    if ready.readable && read_session(entry, slot, generation, worker, owners, request)? {
        close(entry, lifecycle, maildrops)?;
    }
    Ok(())
}
pub(super) fn close(
    entry: &mut Option<SessionLease>,
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    let Some(lease) = entry.take() else {
        return Ok(());
    };
    maildrops.release_generation(lease.get().generation);
    lifecycle
        .close(lease)
        .map_err(|error| io::Error::other(format!("session close failed: {error:?}")))
}
