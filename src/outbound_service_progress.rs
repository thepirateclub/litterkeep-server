//! Учёт завершённых queue jobs и bounded drain budget.

use crate::{
    outbound_service::{DrainReport, Phase, request_due},
    queue::State,
    storage_worker::WakingStorageWorker,
};
use std::io;

/// Записывает итог одного job и запрашивает следующий, пока не исчерпан budget.
pub(super) fn next_job(
    storage: &WakingStorageWorker,
    ids: &mut u64,
    remaining: &mut usize,
    report: &mut DrainReport,
    state: State,
) -> io::Result<Phase> {
    record(report, state);
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| io::Error::other("outbound limit underflow"))?;
    match *remaining {
        0 => Ok(Phase::Finished),
        _ => {
            request_due(storage, ids)?;
            Ok(Phase::Due)
        }
    }
}

fn record(report: &mut DrainReport, state: State) {
    match state {
        State::Delivered => report.delivered += 1,
        State::Deferred => report.deferred += 1,
        State::PermanentFailure => report.permanent_failures += 1,
        State::Pending | State::InFlight => {}
    }
}
