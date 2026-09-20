//! Ограниченный queue drain через собственный SMTP transport.
use crate::{outbound, queue::State, store::Store};
use std::io;

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
    for delivery in store.pending_outbound()?.into_iter().take(limit) {
        store.transition_outbound(&delivery, State::InFlight)?;
        let payload = store.outbound_blob(&delivery)?;
        let state = delivery_state(outbound::deliver(
            address,
            &delivery.sender,
            &delivery.recipient,
            &payload,
        ));
        store.transition_outbound(&delivery, state)?;
        record(&mut report, state);
    }
    Ok(report)
}

/// Отделяет непредсказуемую I/O ошибку peer-а от terminal SMTP ответа.
fn delivery_state(result: io::Result<outbound::DeliveryOutcome>) -> State {
    match result {
        Ok(outbound::DeliveryOutcome::Delivered) => State::Delivered,
        Ok(outbound::DeliveryOutcome::Deferred) | Err(_) => State::Deferred,
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
