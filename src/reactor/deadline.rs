//! Расчёт ближайшего TLS handshake deadline для блокирующего `poll(2)`.
//!
//! Модуль не закрывает сокеты сам: он только вычисляет время ожидания и
//! освобождает просроченный session slot через его единственного владельца.
use super::{MaildropLeases, Orchestrator, Session, SessionLease, close_session};
use std::io;
use std::time::Instant;

/// Возвращает миллисекунды до ближайшего handshake либо бесконечное ожидание.
pub(super) fn poll_timeout(sessions: &[Option<SessionLease>]) -> i32 {
    let nearest = sessions
        .iter()
        .filter_map(Option::as_ref)
        .filter_map(|lease| lease.get().handshake_deadline())
        .min();
    match nearest {
        Some(deadline) => timeout_until(Instant::now(), deadline),
        None => -1,
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
            close_session(entry, lifecycle, maildrops)?;
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
    use std::time::Duration;

    #[test]
    fn deadline_is_rounded_up_and_expiry_is_immediate() {
        let now = Instant::now();
        assert_eq!(timeout_until(now, now), 0);
        assert_eq!(timeout_until(now, now + Duration::from_micros(1_001)), 2);
    }
}
