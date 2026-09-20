//! Применение cooperative cancellation к реальным владельцам клиентских сокетов.

use super::{MaildropLeases, Orchestrator, Session, SessionLease, session_close};
use crate::runtime::TaskState;
use std::io;

/// Закрывает только те сессии, чьи task owner-ы уже переведены в Cancelling.
///
/// Отдельный проход после dispatch нужен потому, что control-команда меняет
/// lifecycle в середине готовой пачки poll-событий. Реальный socket lease
/// освобождается тем же единственным путём, что EOF, timeout и protocol error.
pub(super) fn close_cancelled(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    for entry in sessions {
        let cancelled = entry
            .as_ref()
            .and_then(|lease| lifecycle.task_state(lease.owner()))
            .is_some_and(|state| state == TaskState::Cancelling);
        if cancelled {
            session_close::close(entry, lifecycle, maildrops)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactor::Kind;
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn cancellation_closes_the_owned_socket_and_retires_metadata() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let peer = TcpStream::connect(listener.local_addr().expect("address")).expect("peer");
        let (stream, _) = listener.accept().expect("accepted stream");
        let session = Session::new(Kind::Smtp, stream, 1, None, None).expect("session");
        let mut lifecycle = Orchestrator::new();
        let task = lifecycle.spawn_task();
        let resource = lifecycle.register_resource(session);
        let lease = lifecycle.acquire(task, resource).expect("lease");
        let mut sessions = vec![Some(lease)];

        assert!(lifecycle.request_cancel(task));
        close_cancelled(
            &mut sessions,
            &mut lifecycle,
            &mut MaildropLeases::default(),
        )
        .expect("cancelled close");

        assert!(sessions.iter().all(Option::is_none));
        assert_eq!(lifecycle.task_count(), 0);
        assert_eq!(lifecycle.resource_count(), 0);
        drop(peer);
    }
}
