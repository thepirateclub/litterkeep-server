//! Главный poll-loop SMTP/IMAP; вынесен из composition root reactor-а.

use super::*;

pub(super) fn serve(
    smtp_address: Option<&str>,
    imap_address: Option<&str>,
    pop3_address: Option<&str>,
    managesieve_address: Option<&str>,
    outbound_route: Option<crate::outbound_route::Route>,
    root: impl AsRef<Path>,
    tls_mode: TlsMode,
) -> io::Result<()> {
    let root = root.as_ref();
    let smtp_listener = listener::bind(smtp_address)?;
    let imap_listener = listener::bind(imap_address)?;
    let pop3_listener = listener::bind(pop3_address)?;
    let managesieve_listener = listener::bind(managesieve_address)?;
    let mut worker = WakingStorageWorker::start(root, 64)?;
    let mut lifecycle: Orchestrator<Session> = Orchestrator::new();
    let mut scheduler = match outbound_route {
        Some(route) => Some(outbound_scheduler::OutboundScheduler::start(
            route,
            &worker,
            &mut lifecycle,
        )?),
        None => None,
    };
    let control = crate::control::Listener::bind(root)?;
    let mut sessions: Vec<Option<SessionLease>> = Vec::new();
    let mut generation = 1_u64;
    let mut request = 1_u64;
    let mut owners = HashMap::new();
    let mut maildrops = MaildropLeases::default();
    let mut control_sessions = Vec::new();
    loop {
        step(
            smtp_listener.as_ref(),
            imap_listener.as_ref(),
            pop3_listener.as_ref(),
            managesieve_listener.as_ref(),
            &control,
            &mut control_sessions,
            &tls_mode,
            &mut worker,
            &mut scheduler,
            &mut lifecycle,
            &mut sessions,
            &mut generation,
            &mut request,
            &mut owners,
            &mut maildrops,
        )?;
    }
}

#[allow(clippy::too_many_arguments)]
fn step(
    smtp: Option<&TcpListener>,
    imap: Option<&TcpListener>,
    pop3: Option<&TcpListener>,
    managesieve: Option<&TcpListener>,
    control: &crate::control::Listener,
    control_sessions: &mut Vec<Option<crate::control::Connection>>,
    tls: &TlsMode,
    worker: &mut WakingStorageWorker,
    scheduler: &mut Option<outbound_scheduler::OutboundScheduler>,
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
    if let Some(scheduler) = scheduler.as_ref() {
        targets.push(Target::OutboundWake);
        poll_fds.push((scheduler.wake_reader().as_fd(), readable()));
    }
    listener::append(smtp, Target::SmtpListener, &mut targets, &mut poll_fds);
    listener::append(imap, Target::ImapListener, &mut targets, &mut poll_fds);
    listener::append(pop3, Target::Pop3Listener, &mut targets, &mut poll_fds);
    listener::append(
        managesieve,
        Target::ManageSieveListener,
        &mut targets,
        &mut poll_fds,
    );
    targets.push(Target::ControlListener);
    poll_fds.push((control.as_fd(), readable()));
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
    for (slot, session) in control_sessions.iter().enumerate() {
        if let Some(connection) = session {
            targets.push(Target::ControlSession(slot));
            poll_fds.push((
                connection.as_fd(),
                Interest {
                    readable: true,
                    writable: connection.wants_write(),
                },
            ));
        }
    }
    let ready = linux_poll::wait(
        &poll_fds,
        deadline::poll_timeout(
            sessions,
            scheduler.as_ref().and_then(|value| value.deadline()),
        ),
    )?;
    deadline::expire_handshakes(sessions, lifecycle, maildrops)?;
    let stop_scheduler = match scheduler.as_mut() {
        Some(value) => value.wake_due(worker, lifecycle)?,
        None => false,
    };
    if stop_scheduler {
        let Some(value) = scheduler.take() else {
            return Err(io::Error::other(
                "scheduler disappeared during cancellation",
            ));
        };
        value.shutdown(lifecycle)?;
    }
    for (ready, target) in ready.into_iter().zip(targets) {
        dispatch(
            ready,
            target,
            smtp,
            imap,
            pop3,
            managesieve,
            control,
            control_sessions,
            tls,
            worker,
            scheduler,
            lifecycle,
            sessions,
            generation,
            request,
            owners,
            maildrops,
        )?;
    }
    session_cancel::close_cancelled(sessions, lifecycle, maildrops)?;
    session::termination::close_terminated(sessions, lifecycle, maildrops)
}

fn readable() -> Interest {
    Interest {
        readable: true,
        writable: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch(
    ready: crate::platform::linux_poll::Ready,
    target: Target,
    smtp: Option<&TcpListener>,
    imap: Option<&TcpListener>,
    pop3: Option<&TcpListener>,
    managesieve: Option<&TcpListener>,
    control: &crate::control::Listener,
    control_sessions: &mut Vec<Option<crate::control::Connection>>,
    tls: &TlsMode,
    worker: &mut WakingStorageWorker,
    scheduler: &mut Option<outbound_scheduler::OutboundScheduler>,
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
        Target::ControlListener if ready.readable => control.accept(control_sessions),
        Target::WorkerWake if ready.readable => {
            worker.drain_wakeup()?;
            let unclaimed = complete(worker, sessions, owners, request, maildrops)?;
            for response in unclaimed {
                let Some(scheduler) = scheduler.as_mut() else {
                    return Err(io::Error::other("unclaimed storage completion"));
                };
                scheduler.storage_completed(worker, response, lifecycle)?;
            }
            retry_submissions(worker, sessions, owners)
        }
        Target::OutboundWake if ready.readable => {
            let Some(scheduler) = scheduler.as_mut() else {
                return Err(io::Error::other("outbound wake without scheduler"));
            };
            scheduler.transport_completed(worker, lifecycle)
        }
        Target::Session(slot, generation) => service_session(
            ready, slot, generation, worker, lifecycle, sessions, owners, request, maildrops,
        ),
        Target::ControlSession(slot) => service_control(ready, slot, control_sessions, lifecycle),
        _ => Ok(()),
    }
}

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

#[allow(clippy::too_many_arguments)]
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
        return session_close::close(entry, lifecycle, maildrops);
    }
    match ready.writable {
        true if flush(entry)? => {
            session_close::close(entry, lifecycle, maildrops)?;
            return Ok(());
        }
        true | false => {}
    }
    match ready.readable {
        true if read_session(entry, slot, generation, worker, owners, request)? => {
            session_close::close(entry, lifecycle, maildrops)?;
        }
        true | false => {}
    }
    Ok(())
}
