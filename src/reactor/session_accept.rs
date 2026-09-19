//! Принятие TCP-сессий и выдача им lifecycle lease.

use super::*;

/// Принимает ограниченную пачку клиентов, сохраняя одного владельца socket.
pub(super) fn accept(
    listener: &TcpListener,
    kind: Kind,
    sessions: &mut Vec<Option<SessionLease>>,
    next: &mut u64,
    tls_mode: &TlsMode,
    lifecycle: &mut Orchestrator<Session>,
) -> io::Result<()> {
    for _ in 0..ACCEPT_BUDGET {
        match listener.accept() {
            Ok((stream, _)) if lifecycle.resource_count() < MAX_SESSIONS => {
                let (implicit, upgrade) = match (tls_mode, kind) {
                    (TlsMode::Implicit(config), _) => (Some(config.clone()), None),
                    (TlsMode::StartTls(config), _) => (None, Some(config.clone())),
                    (TlsMode::Plain, _) => (None, None),
                };
                let session = Session::new(kind, stream, *next, implicit, upgrade)?;
                let task = lifecycle.spawn_task();
                let resource = lifecycle.register_resource(session);
                let session = lifecycle.acquire(task, resource).map_err(|error| {
                    io::Error::other(format!("session lease failed: {error:?}"))
                })?;
                *next = next.wrapping_add(1).max(1);
                place(sessions, session)?;
            }
            Ok((stream, _)) => drop(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Переиспользует освобождённый slot и не раздувает таблицу сессий holes.
fn place(sessions: &mut Vec<Option<SessionLease>>, session: SessionLease) -> io::Result<()> {
    if let Some(slot) = sessions.iter().position(Option::is_none) {
        let Some(entry) = sessions.get_mut(slot) else {
            return Err(io::Error::other("missing reusable session slot"));
        };
        *entry = Some(session);
    } else {
        sessions.push(Some(session));
    }
    Ok(())
}
