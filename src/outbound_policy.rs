//! Политика переходов durable исходящей очереди после SMTP попытки.
//!
//! Этот модуль не владеет ни Store, ни socket. Он чисто переводит проверенный
//! transport outcome в записываемый queue state, retry deadline и DSN cause.

use crate::{dsn, outbound::DeliveryOutcome, queue::State};
use std::io;

/// Максимум действительно начатых попыток для одного recipient job.
pub const MAX_ATTEMPTS: u8 = 3;
const INITIAL_RETRY_SECONDS: u64 = 60;
const MAX_RETRY_SECONDS: u64 = 3_600;

/// Выбирает durable state после одной завершённой SMTP попытки.
pub fn state(result: &io::Result<DeliveryOutcome>, attempts: u8) -> State {
    match result {
        Ok(DeliveryOutcome::Delivered) => State::Delivered,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => State::PermanentFailure,
        Ok(DeliveryOutcome::Deferred) | Err(_) => match attempts >= MAX_ATTEMPTS {
            true => State::PermanentFailure,
            false => State::Deferred,
        },
        Ok(DeliveryOutcome::PermanentFailure) => State::PermanentFailure,
    }
}

/// Возвращает restart-safe Unix epoch deadline для нетерминального retry.
pub fn next_attempt_epoch(state: State, attempts: u8, now_epoch: u64) -> u64 {
    match state {
        State::Deferred => {
            let shift = u32::from(attempts.saturating_sub(1)).min(6);
            let delay = INITIAL_RETRY_SECONDS
                .saturating_mul(1_u64 << shift)
                .min(MAX_RETRY_SECONDS);
            now_epoch.saturating_add(delay)
        }
        State::Pending | State::InFlight | State::Delivered | State::PermanentFailure => 0,
    }
}

/// Строит typed DSN cause только для terminal failure текущего job.
pub fn dsn_failure(result: &io::Result<DeliveryOutcome>, attempts: u8) -> Option<dsn::Failure> {
    match (result, attempts >= MAX_ATTEMPTS) {
        (Ok(DeliveryOutcome::Delivered), _) => None,
        (Ok(DeliveryOutcome::PermanentFailure), _) => Some(dsn::Failure::PermanentSmtp),
        (Err(error), _) if error.kind() == io::ErrorKind::InvalidInput => {
            Some(dsn::Failure::RoutePolicy)
        }
        (Ok(DeliveryOutcome::Deferred) | Err(_), true) => Some(dsn::Failure::RetryExhausted),
        (Ok(DeliveryOutcome::Deferred) | Err(_), false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_deferred_attempt_becomes_permanent_failure() {
        assert_eq!(
            state(&Ok(DeliveryOutcome::Deferred), MAX_ATTEMPTS),
            State::PermanentFailure
        );
    }

    #[test]
    fn deferred_retry_uses_bounded_epoch_backoff() {
        assert_eq!(next_attempt_epoch(State::Deferred, 1, 100), 160);
        assert_eq!(next_attempt_epoch(State::Deferred, 3, 100), 340);
        assert_eq!(next_attempt_epoch(State::Delivered, 1, 100), 0);
    }

    /// Фиксирует весь экспоненциальный ряд и его безопасную верхнюю границу.
    #[test]
    fn deferred_retry_backoff_doubles_and_saturates() {
        let expected = [60, 120, 240, 480, 960, 1_920, 3_600, 3_600];
        for (attempt, delay) in (1_u8..=8).zip(expected) {
            assert_eq!(
                next_attempt_epoch(State::Deferred, attempt, 0),
                delay,
                "unexpected retry delay at attempt {attempt}"
            );
        }
    }

    /// Переполнение Unix epoch не превращает очередь в немедленно due job.
    #[test]
    fn deferred_retry_deadline_saturates_at_epoch_maximum() {
        assert_eq!(
            next_attempt_epoch(State::Deferred, 1, u64::MAX - 10),
            u64::MAX
        );
    }

    #[test]
    fn terminal_outcomes_choose_typed_dsn_reason() {
        assert_eq!(
            dsn_failure(&Ok(DeliveryOutcome::PermanentFailure), 1),
            Some(dsn::Failure::PermanentSmtp)
        );
        assert_eq!(
            dsn_failure(&Ok(DeliveryOutcome::Deferred), MAX_ATTEMPTS),
            Some(dsn::Failure::RetryExhausted)
        );
        assert_eq!(dsn_failure(&Ok(DeliveryOutcome::Delivered), 1), None);
        let denied = Err(io::Error::new(io::ErrorKind::InvalidInput, "denied"));
        assert_eq!(state(&denied, 1), State::PermanentFailure);
        assert_eq!(dsn_failure(&denied, 1), Some(dsn::Failure::RoutePolicy));
    }
}
