//! Расчёт ближайшего TLS handshake deadline для блокирующего `poll(2)`.
//!
//! Модуль не закрывает сокеты сам: он только вычисляет время ожидания и
//! освобождает просроченный session slot через его единственного владельца.
use super::{MaildropLeases, Orchestrator, Session, SessionLease};
use std::io;
use std::time::{Duration, Instant};

/// RFC 5321 §4.5.3.2.7 SMTP server inactivity interval.
const SMTP_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// Возвращает миллисекунды до ближайшего handshake, SMTP idle либо scheduler deadline.
pub(super) fn poll_timeout(
    sessions: &[Option<SessionLease>],
    scheduler_deadline: Option<Instant>,
) -> i32 {
    let handshake = sessions
        .iter()
        .filter_map(Option::as_ref)
        .filter_map(|lease| lease.get().handshake_deadline())
        .min();
    let smtp_idle = sessions
        .iter()
        .filter_map(Option::as_ref)
        .filter_map(|lease| smtp_idle_deadline(lease.get(), SMTP_IDLE_TIMEOUT))
        .min();
    let nearest = [handshake, smtp_idle, scheduler_deadline]
        .into_iter()
        .flatten()
        .min();
    match nearest {
        Some(deadline) => timeout_until(Instant::now(), deadline),
        None => -1,
    }
}

/// Закрывает просроченные SMTP session общим lifecycle-путём освобождения lease.
pub(super) fn expire_smtp_idle(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    expire_smtp_idle_at(
        sessions,
        lifecycle,
        maildrops,
        Instant::now(),
        SMTP_IDLE_TIMEOUT,
    )
}

/// Вычисляет и применяет timeout с явным временем для детерминированного теста.
fn expire_smtp_idle_at(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
    now: Instant,
    timeout: Duration,
) -> io::Result<()> {
    for entry in sessions {
        let expired = entry
            .as_ref()
            .and_then(|lease| smtp_idle_deadline(lease.get(), timeout))
            .is_some_and(|deadline| deadline <= now);
        if expired {
            super::session_close::close(entry, lifecycle, maildrops)?;
        }
    }
    Ok(())
}

/// Таймер действует только на SMTP, не занятую TLS, worker-ом или отправкой.
fn smtp_idle_deadline(session: &Session, timeout: Duration) -> Option<Instant> {
    let eligible = matches!(
        (
            session.kind,
            session.pending.is_some(),
            session.pending_submit.is_some(),
            session.output.is_empty(),
            session.wire_output.is_empty(),
            session.handshake_deadline().is_some(),
            session
                .tls
                .as_ref()
                .is_some_and(super::ServerTls::wants_write),
        ),
        (super::Kind::Smtp, false, false, true, true, false, false),
    );
    match eligible {
        true => session.last_activity.checked_add(timeout),
        false => None,
    }
}

/// Удаляет только TLS-сессии, не завершившие handshake до своего deadline.
pub(super) fn expire_handshakes(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    let now = Instant::now();
    for entry in sessions {
        let expired = entry
            .as_ref()
            .is_some_and(|lease| lease.get().handshake_expired(now));
        if expired {
            super::session_close::close(entry, lifecycle, maildrops)?;
        }
    }
    Ok(())
}

/// Округляет положительную долю миллисекунды вверх, избегая busy-loop.
fn timeout_until(now: Instant, deadline: Instant) -> i32 {
    let remaining = deadline.saturating_duration_since(now);
    if remaining.is_zero() {
        return 0;
    }
    let milliseconds = remaining.as_millis();
    let rounded = match remaining.subsec_nanos() % 1_000_000 {
        0 => milliseconds,
        _ => milliseconds.saturating_add(1),
    };
    rounded.min(i32::MAX as u128) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactor::{Kind, Pending, Session};
    use crate::runtime::Orchestrator;
    use std::net::{TcpListener, TcpStream};

    /// Создаёт SMTP lease с реальным socket-ресурсом lifecycle-оркестратора.
    fn smtp_lease(lifecycle: &mut Orchestrator<Session>) -> (SessionLease, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let peer = TcpStream::connect(listener.local_addr().expect("address")).expect("peer");
        let (stream, _) = listener.accept().expect("accept");
        let mut session = Session::new(Kind::Smtp, stream, 1, None, None).expect("session");
        session.output.clear();
        let task = lifecycle.spawn_task();
        let resource = lifecycle.register_resource(session);
        let lease = lifecycle.acquire(task, resource).expect("lease");
        (lease, peer)
    }
    use std::time::Duration;

    #[test]
    fn deadline_is_rounded_up_and_expiry_is_immediate() {
        let now = Instant::now();
        assert_eq!(timeout_until(now, now), 0);
        assert_eq!(timeout_until(now, now + Duration::from_micros(1_001)), 2);
    }

    #[test]
    fn smtp_idle_lease_is_reclaimed_at_exact_timeout_boundary() {
        let mut lifecycle = Orchestrator::new();
        let (mut lease, _peer) = smtp_lease(&mut lifecycle);
        let started = Instant::now();
        lease.get_mut().last_activity = started;
        let mut sessions = vec![Some(lease)];
        let mut maildrops = MaildropLeases::default();

        expire_smtp_idle_at(
            &mut sessions,
            &mut lifecycle,
            &mut maildrops,
            started + Duration::from_secs(299),
            SMTP_IDLE_TIMEOUT,
        )
        .expect("not yet expired");
        assert_eq!(lifecycle.task_count(), 1);
        expire_smtp_idle_at(
            &mut sessions,
            &mut lifecycle,
            &mut maildrops,
            started + SMTP_IDLE_TIMEOUT,
            SMTP_IDLE_TIMEOUT,
        )
        .expect("expire idle lease");

        assert!(sessions.iter().all(Option::is_none));
        assert_eq!(lifecycle.task_count(), 0);
        assert_eq!(lifecycle.resource_count(), 0);
    }

    #[test]
    fn smtp_idle_expiry_does_not_cancel_pending_storage_owner() {
        let mut lifecycle = Orchestrator::new();
        let (mut lease, _peer) = smtp_lease(&mut lifecycle);
        let started = Instant::now();
        lease.get_mut().last_activity = started;
        lease.get_mut().pending = Some(Pending::Pop3ReadInbox);
        let mut sessions = vec![Some(lease)];

        expire_smtp_idle_at(
            &mut sessions,
            &mut lifecycle,
            &mut MaildropLeases::default(),
            started + SMTP_IDLE_TIMEOUT,
            SMTP_IDLE_TIMEOUT,
        )
        .expect("busy lease remains owned");

        assert!(sessions.iter().any(Option::is_some));
        assert_eq!(lifecycle.task_count(), 1);
        assert_eq!(lifecycle.resource_count(), 1);
    }
}
