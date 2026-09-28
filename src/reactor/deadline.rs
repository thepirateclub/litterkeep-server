//! Расчёт ближайших TLS handshake и SMTP inactivity deadlines для `poll(2)`.
//!
//! Модуль не закрывает сокеты сам: он только вычисляет время ожидания и
//! освобождает просроченный session slot через его единственного владельца.
use super::{MaildropLeases, Orchestrator, Session, SessionLease};
use std::io;
use std::time::{Duration, Instant};

/// RFC 5321 §4.5.3.2.7 SMTP command inactivity deadline.
const SMTP_COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
/// RFC 5321 §4.5.3.2.5 SMTP DATA block wait.
const SMTP_DATA_BLOCK_TIMEOUT: Duration = Duration::from_secs(180);

/// Рассчитывает ожидание до ближайшего TLS, SMTP либо scheduler deadline.
pub(super) fn poll_timeout(
    sessions: &[Option<SessionLease>],
    scheduler_deadline: Option<Instant>,
) -> i32 {
    let handshake = sessions
        .iter()
        .filter_map(Option::as_ref)
        .filter_map(|lease| lease.get().handshake_deadline())
        .min();
    let smtp = sessions
        .iter()
        .filter_map(Option::as_ref)
        .filter_map(|lease| smtp_deadline(lease.get()))
        .min();
    let nearest = [handshake, smtp, scheduler_deadline]
        .into_iter()
        .flatten()
        .min();
    match nearest {
        Some(deadline) => timeout_until(Instant::now(), deadline),
        None => -1,
    }
}

/// Закрывает SMTP lease по фазовому timeout общим lifecycle-путём.
pub(super) fn expire_smtp(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    expire_smtp_at(sessions, lifecycle, maildrops, Instant::now())
}

/// Вычисляет и применяет timeout с явным временем для детерминированного теста.
fn expire_smtp_at(
    sessions: &mut [Option<SessionLease>],
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
    now: Instant,
) -> io::Result<()> {
    for entry in sessions {
        let expired = entry
            .as_ref()
            .and_then(|lease| smtp_deadline(lease.get()))
            .is_some_and(|deadline| deadline <= now);
        if expired {
            super::session_close::close(entry, lifecycle, maildrops)?;
        }
    }
    Ok(())
}

/// Выбирает SMTP command или DATA phase, исключая TLS и незавершённый worker.
fn smtp_deadline(session: &Session) -> Option<Instant> {
    let state = match (
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
        matches!(
            session.smtp,
            super::SmtpState::Data { .. } | super::SmtpState::BdatReceiving(_)
        ),
    ) {
        (super::Kind::Smtp, false, false, true, true, false, false, true) => {
            Some((session.smtp_data_activity, SMTP_DATA_BLOCK_TIMEOUT))
        }
        (super::Kind::Smtp, false, false, true, true, false, false, false) => {
            Some((session.last_activity, SMTP_COMMAND_TIMEOUT))
        }
        _ => None,
    }?;
    state.0.checked_add(state.1)
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
    use crate::reactor::session::DataSection;
    use crate::reactor::{Kind, Pending, Session, SmtpState};
    use crate::runtime::Orchestrator;
    use crate::storage_worker::WakingStorageWorker;
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

    /// Переводит session в DATA и проверяет, что свежая строка обновила блок.
    fn begin_data(session: &mut Session, started: Instant) {
        session.smtp_data_activity = started;
        session.smtp = SmtpState::Data {
            bytes: Vec::new(),
            section: DataSection::Headers,
            recipients: vec!["bob@litterkeep.test".into()],
            reverse_path: "alice@litterkeep.test".into(),
            dsn: crate::smtp::DsnEnvelope {
                reverse_path: "alice@litterkeep.test".into(),
                mail: Default::default(),
                recipients: vec![crate::smtp::RcptTo {
                    address: "bob@litterkeep.test".into(),
                    dsn: Default::default(),
                }],
            },
            oversized: false,
            invalid_octets: false,
            body_mode: crate::smtp::BodyMode::SevenBit,
            smtp_utf8: false,
        };
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
        let mut sessions = [Some(lease)];
        let mut maildrops = MaildropLeases::default();

        expire_smtp_at(
            &mut sessions,
            &mut lifecycle,
            &mut maildrops,
            started + Duration::from_secs(299),
        )
        .expect("not yet expired");
        assert_eq!(lifecycle.task_count(), 1);
        expire_smtp_at(
            &mut sessions,
            &mut lifecycle,
            &mut maildrops,
            started + SMTP_COMMAND_TIMEOUT,
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
        let mut sessions = [Some(lease)];

        expire_smtp_at(
            &mut sessions,
            &mut lifecycle,
            &mut MaildropLeases::default(),
            started + SMTP_COMMAND_TIMEOUT,
        )
        .expect("busy lease remains owned");

        assert!(sessions.iter().any(Option::is_some));
        assert_eq!(lifecycle.task_count(), 1);
        assert_eq!(lifecycle.resource_count(), 1);
    }

    #[test]
    fn smtp_data_deadline_is_three_minutes_and_reclaims_its_owner() {
        let mut lifecycle = Orchestrator::new();
        let (mut lease, _peer) = smtp_lease(&mut lifecycle);
        let started = Instant::now();
        lease.get_mut().last_activity = started;
        begin_data(lease.get_mut(), started);
        assert_eq!(
            smtp_deadline(lease.get()),
            Some(started + SMTP_DATA_BLOCK_TIMEOUT)
        );
        let mut sessions = [Some(lease)];

        expire_smtp_at(
            &mut sessions,
            &mut lifecycle,
            &mut MaildropLeases::default(),
            started + Duration::from_secs(179),
        )
        .expect("DATA deadline has not elapsed");
        assert!(sessions.iter().any(Option::is_some));
        expire_smtp_at(
            &mut sessions,
            &mut lifecycle,
            &mut MaildropLeases::default(),
            started + SMTP_DATA_BLOCK_TIMEOUT,
        )
        .expect("expired DATA block releases lease");
        assert!(sessions.iter().all(Option::is_none));
        assert_eq!(lifecycle.resource_count(), 0);
    }

    #[test]
    fn stale_complete_lines_cannot_refresh_an_incomplete_data_block() {
        let mut lifecycle = Orchestrator::new();
        let (mut lease, _peer) = smtp_lease(&mut lifecycle);
        let started = Instant::now();
        begin_data(lease.get_mut(), started);
        lease.get_mut().input = b"Subject: x\r\n\r\npartial".to_vec();
        let mut sessions = [Some(lease)];
        let mut worker = WakingStorageWorker::start(std::env::temp_dir(), 2).expect("worker");
        let mut owners = std::collections::HashMap::new();
        let mut request_id = 1;

        let session = sessions
            .first_mut()
            .and_then(Option::as_mut)
            .expect("lease")
            .get_mut();
        super::super::protocol::process_input(
            session,
            0,
            1,
            &mut worker,
            &mut owners,
            &mut request_id,
        )
        .expect("consume complete lines");
        let expired = smtp_deadline(session)
            .is_some_and(|deadline| deadline <= started + SMTP_DATA_BLOCK_TIMEOUT);
        assert!(expired, "old lines must not refresh the partial DATA block");
        worker.shutdown();
    }
}
