//! Ограниченный queue drain через собственный SMTP transport.
use crate::{outbound, outbound_pool::ConnectionPool, queue::State, store::Store};
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ATTEMPTS: u8 = 3;
const ROUTE_POOL_CAPACITY: usize = 2;
const INITIAL_RETRY_SECONDS: u64 = 60;
const MAX_RETRY_SECONDS: u64 = 3_600;

/// Итог одной bounded попытки доставки без скрытого бесконечного retry loop.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct DrainReport {
    pub delivered: usize,
    pub deferred: usize,
    pub permanent_failures: usize,
}

/// Обрабатывает не больше `limit` committed jobs владельцем одного Store.
pub fn drain(store: &mut Store, address: &str, limit: usize) -> io::Result<DrainReport> {
    drain_at(store, address, limit, now_epoch()?)
}

/// Обрабатывает due jobs на заданном epoch для детерминированной проверки policy.
pub fn drain_at(
    store: &mut Store,
    address: &str,
    limit: usize,
    now_epoch: u64,
) -> io::Result<DrainReport> {
    let mut report = DrainReport::default();
    let mut pool = ConnectionPool::new(address, ROUTE_POOL_CAPACITY)?;
    for mut delivery in store.due_outbound(now_epoch)?.into_iter().take(limit) {
        delivery.attempts = delivery
            .attempts
            .checked_add(1)
            .ok_or_else(|| io::Error::other("outbound attempts exhausted"))?;
        delivery.state = State::InFlight;
        store.transition_outbound(&delivery, State::InFlight)?;
        let payload = store.outbound_blob(&delivery)?;
        let state = delivery_state(
            pool.deliver(&delivery.sender, &delivery.recipient, &payload),
            delivery.attempts,
        );
        delivery.state = state;
        delivery.next_attempt_epoch = next_attempt_epoch(state, delivery.attempts, now_epoch);
        store.transition_outbound(&delivery, state)?;
        record(&mut report, state);
    }
    Ok(report)
}

/// Возвращает durable wall-clock deadline, пригодный для restart процесса.
fn now_epoch() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| io::Error::other("system clock predates Unix epoch"))
}

/// Ограничивает retry delay, чтобы счётчик попыток не переполнил duration.
fn next_attempt_epoch(state: State, attempts: u8, now_epoch: u64) -> u64 {
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

/// Отделяет непредсказуемую I/O ошибку peer-а от terminal SMTP ответа.
fn delivery_state(result: io::Result<outbound::DeliveryOutcome>, attempts: u8) -> State {
    match result {
        Ok(outbound::DeliveryOutcome::Delivered) => State::Delivered,
        Ok(outbound::DeliveryOutcome::Deferred) | Err(_) => match attempts >= MAX_ATTEMPTS {
            true => State::PermanentFailure,
            false => State::Deferred,
        },
        Ok(outbound::DeliveryOutcome::PermanentFailure) => State::PermanentFailure,
    }
}

/// Считает только durable terminal result одной реально начатой попытки.
fn record(report: &mut DrainReport, state: State) {
    match state {
        State::Delivered => report.delivered += 1,
        State::Deferred => report.deferred += 1,
        State::PermanentFailure => report.permanent_failures += 1,
        State::Pending | State::InFlight => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn third_deferred_attempt_becomes_permanent_failure() {
        assert_eq!(
            delivery_state(Ok(outbound::DeliveryOutcome::Deferred), MAX_ATTEMPTS),
            State::PermanentFailure
        );
    }

    #[test]
    fn deferred_retry_uses_bounded_epoch_backoff() {
        assert_eq!(next_attempt_epoch(State::Deferred, 1, 100), 160);
        assert_eq!(next_attempt_epoch(State::Deferred, 3, 100), 340);
        assert_eq!(next_attempt_epoch(State::Delivered, 1, 100), 0);
    }
}
