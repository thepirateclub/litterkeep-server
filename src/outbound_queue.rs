//! Ограниченный queue drain через собственный SMTP transport.
use crate::{outbound, outbound_pool::ConnectionPool, queue::State, store::Store};
use std::io;

const MAX_ATTEMPTS: u8 = 3;
const ROUTE_POOL_CAPACITY: usize = 2;

/// Итог одной bounded попытки доставки без скрытого бесконечного retry loop.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct DrainReport {
    pub delivered: usize,
    pub deferred: usize,
    pub permanent_failures: usize,
}

/// Обрабатывает не больше `limit` committed jobs владельцем одного Store.
pub fn drain(store: &mut Store, address: &str, limit: usize) -> io::Result<DrainReport> {
    let mut report = DrainReport::default();
    let mut pool = ConnectionPool::new(address, ROUTE_POOL_CAPACITY)?;
    for mut delivery in store.pending_outbound()?.into_iter().take(limit) {
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
        store.transition_outbound(&delivery, state)?;
        record(&mut report, state);
    }
    Ok(report)
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
}
