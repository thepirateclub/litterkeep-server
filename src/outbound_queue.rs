//! Ограниченный queue drain через собственный SMTP transport.
use crate::{
    accounts::Accounts, dsn, local_route, outbound_policy, outbound_pool::ConnectionPool,
    queue::State, store::Store,
};
use std::{
    io,
    time::{SystemTime, UNIX_EPOCH},
};

const ROUTE_POOL_CAPACITY: usize = 2;

/// Итог одной bounded попытки доставки без скрытого бесконечного retry loop.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct DrainReport {
    pub delivered: usize,
    pub deferred: usize,
    pub permanent_failures: usize,
}

/// Обрабатывает не больше `limit` committed jobs владельцем одного Store.
pub fn drain(
    store: &mut Store,
    accounts: &Accounts,
    address: &str,
    limit: usize,
) -> io::Result<DrainReport> {
    drain_at(store, accounts, address, limit, now_epoch()?)
}

/// Обрабатывает due jobs на заданном epoch для детерминированной проверки policy.
pub fn drain_at(
    store: &mut Store,
    accounts: &Accounts,
    address: &str,
    limit: usize,
    now_epoch: u64,
) -> io::Result<DrainReport> {
    let mut report = DrainReport::default();
    let mut pool = ConnectionPool::new(address, ROUTE_POOL_CAPACITY)?;
    let result = (|| {
        for mut delivery in store.due_outbound(now_epoch)?.into_iter().take(limit) {
            let payload = store.outbound_blob(&delivery)?;
            if let Some(failure) = delivery.dsn_failure {
                publish_dsn(store, accounts, &delivery, &payload, failure)?;
                delivery.dsn_failure = None;
                delivery.dsn_diagnostic = None;
                store.transition_outbound(&delivery, delivery.state)?;
                record(&mut report, delivery.state);
                continue;
            }
            delivery.attempts = delivery
                .attempts
                .checked_add(1)
                .ok_or_else(|| io::Error::other("outbound attempts exhausted"))?;
            delivery.state = State::InFlight;
            store.transition_outbound(&delivery, State::InFlight)?;
            let attempt = pool.deliver_detailed(&delivery.sender, &delivery.recipient, &payload);
            let (state, dsn_failure, diagnostic) = match attempt {
                Ok(attempt) => {
                    let result = Ok(attempt.outcome);
                    let state = outbound_policy::state(&result, delivery.attempts);
                    let failure = outbound_policy::requested_dsn_failure(
                        &result,
                        delivery.attempts,
                        delivery.dsn_metadata.as_deref(),
                    );
                    let diagnostic = match failure {
                        Some(_) => attempt.diagnostic,
                        None => None,
                    };
                    (state, failure, diagnostic)
                }
                Err(error) => {
                    let result = Err(error);
                    let state = outbound_policy::state(&result, delivery.attempts);
                    let failure = outbound_policy::requested_dsn_failure(
                        &result,
                        delivery.attempts,
                        delivery.dsn_metadata.as_deref(),
                    );
                    (state, failure, None)
                }
            };
            delivery.state = state;
            delivery.dsn_failure = dsn_failure;
            delivery.dsn_diagnostic = diagnostic;
            delivery.next_attempt_epoch =
                outbound_policy::next_attempt_epoch(state, delivery.attempts, now_epoch);
            store.transition_outbound(&delivery, state)?;
            if let Some(failure) = dsn_failure {
                publish_dsn(store, accounts, &delivery, &payload, failure)?;
                delivery.dsn_failure = None;
                delivery.dsn_diagnostic = None;
                store.transition_outbound(&delivery, state)?;
            }
            record(&mut report, state);
        }
        Ok(report)
    })();
    let shutdown = pool.shutdown();
    match (result, shutdown) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(primary), Err(shutdown)) => Err(io::Error::other(format!(
            "outbound queue drain failed: {primary}; SMTP shutdown failed: {shutdown}"
        ))),
    }
}

/// Возвращает durable wall-clock deadline, пригодный для restart процесса.
fn now_epoch() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| io::Error::other("system clock predates Unix epoch"))
}

/// Публикует DSN только после durable terminal queue state.
///
/// Accounts остаётся policy owner: Store не создаёт mailbox по local-part
/// внешнего адреса. Пустой и внешний reverse-path возвращают `None` до записи.
fn publish_dsn(
    store: &mut Store,
    accounts: &Accounts,
    delivery: &crate::queue::OwnedDelivery,
    payload: &[u8],
    failure: dsn::Failure,
) -> io::Result<()> {
    let target = match (
        local_route::local_mailbox(&delivery.sender),
        delivery.sender.is_empty(),
    ) {
        (Some(mailbox), _) if accounts.exists(&mailbox) => {
            crate::dsn_target::DsnTarget::LocalMailbox(mailbox)
        }
        (Some(_), _) | (None, true) => return Ok(()),
        (None, false) => crate::dsn_target::DsnTarget::OutboundAddress(delivery.sender.clone()),
    };
    let Some(report) = dsn::render_with_diagnostic(
        &delivery.sender,
        &delivery.recipient,
        delivery.transaction_id,
        payload,
        failure,
        delivery.dsn_metadata.as_deref(),
        delivery.dsn_diagnostic.as_ref(),
    ) else {
        return Ok(());
    };
    let Some(key) = delivery.dsn_report_key() else {
        return Err(io::Error::other(
            "terminal DSN notice has no idempotency key",
        ));
    };
    store
        .append_dsn_report_once(target, &report, &key)
        .map(|_| ())
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
