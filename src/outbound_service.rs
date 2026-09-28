//! Event-driven bounded drain durable исходящей очереди на Linux.

use crate::{
    dsn, outbound_policy,
    outbound_service_support::{next_id, now_epoch, submit},
    outbound_wakeup::WakingOutboundWorker,
    platform::linux_poll::{self, Interest},
    queue::{OwnedDelivery, State},
    storage_worker::{Request, Response, WakingStorageWorker},
};
use std::io;
use std::os::fd::AsFd;

/// Итог конечных durable исходов одного bounded service drain.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct DrainReport {
    pub delivered: usize,
    pub deferred: usize,
    pub permanent_failures: usize,
}

/// Дренирует не более `limit` jobs через event-driven Store и SMTP owners.
pub fn drain(
    root: impl AsRef<std::path::Path>,
    address: &str,
    limit: usize,
) -> io::Result<DrainReport> {
    if limit == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "outbound limit must be positive",
        ));
    }
    let mut storage = WakingStorageWorker::start(root, 1)?;
    let mut outbound = WakingOutboundWorker::start(address, 1, 2)?;
    let result = run(&mut storage, &mut outbound, limit);
    let shutdown = outbound.shutdown();
    storage.shutdown();
    match (result, shutdown) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(primary), Err(shutdown)) => Err(io::Error::other(format!(
            "outbound drain failed: {primary}; SMTP shutdown failed: {shutdown}"
        ))),
    }
}

/// Держит только одну delivery между Store и transport, сохраняя простой ownership.
fn run(
    storage: &mut WakingStorageWorker,
    outbound: &mut WakingOutboundWorker,
    limit: usize,
) -> io::Result<DrainReport> {
    let mut report = DrainReport::default();
    let mut ids = 1_u64;
    let mut remaining = limit;
    request_due(storage, &mut ids)?;
    let mut phase = Phase::Due;
    loop {
        let ready = linux_poll::wait(
            &[
                (storage.wake_reader().as_fd(), readable()),
                (outbound.wake_reader().as_fd(), readable()),
            ],
            -1,
        )?;
        for (index, item) in ready.into_iter().enumerate() {
            match (index, item.readable) {
                (0, true) => storage.drain_wakeup()?,
                (1, true) => outbound.drain_wakeup()?,
                (_, false) => {}
                _ => return Err(io::Error::other("unknown service wake target")),
            }
        }
        while let Ok(response) = storage.worker.try_response() {
            phase = handle_storage(
                storage,
                outbound,
                phase,
                response,
                &mut ids,
                &mut remaining,
                &mut report,
            )?;
            if matches!(phase, Phase::Finished) {
                return Ok(report);
            }
        }
        while let Ok(completion) = outbound.worker.try_completion() {
            phase = handle_transport(storage, phase, completion, &mut ids)?;
        }
    }
}

/// Явно описывает, какой durable completion сейчас принадлежит service drain.
enum Phase {
    Due,
    Blob(OwnedDelivery),
    Start(OwnedDelivery, Vec<u8>),
    Transport(OwnedDelivery, Vec<u8>),
    Finalize(OwnedDelivery, Vec<u8>, Option<dsn::Failure>),
    Dsn(State),
    Finished,
}

fn readable() -> Interest {
    Interest {
        readable: true,
        writable: false,
    }
}

fn request_due(storage: &WakingStorageWorker, ids: &mut u64) -> io::Result<()> {
    let request_id = next_id(ids)?;
    submit(
        storage,
        Request::OutboundDue {
            request_id,
            now_epoch: now_epoch()?,
            limit: 1,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_storage(
    storage: &WakingStorageWorker,
    outbound: &mut WakingOutboundWorker,
    phase: Phase,
    response: Response,
    ids: &mut u64,
    remaining: &mut usize,
    report: &mut DrainReport,
) -> io::Result<Phase> {
    match (phase, response) {
        (
            Phase::Due,
            Response::OutboundDue {
                result: Ok(mut jobs),
                ..
            },
        ) => match jobs.pop() {
            None => Ok(Phase::Finished),
            Some(delivery) => {
                submit(
                    storage,
                    Request::OutboundBlob {
                        request_id: next_id(ids)?,
                        delivery: delivery.clone(),
                    },
                )?;
                Ok(Phase::Blob(delivery))
            }
        },
        (
            Phase::Blob(mut delivery),
            Response::OutboundBlob {
                result: Ok(bytes), ..
            },
        ) => {
            delivery.attempts = delivery
                .attempts
                .checked_add(1)
                .ok_or_else(|| io::Error::other("outbound attempts exhausted"))?;
            delivery.state = State::InFlight;
            submit(
                storage,
                Request::OutboundTransition {
                    request_id: next_id(ids)?,
                    delivery: delivery.clone(),
                    state: State::InFlight,
                },
            )?;
            Ok(Phase::Start(delivery, bytes))
        }
        (Phase::Start(delivery, bytes), Response::OutboundTransitioned { result: Ok(()), .. }) => {
            let job_id = next_id(ids)?;
            outbound
                .worker
                .submit(crate::outbound_worker::Job {
                    id: job_id,
                    sender: delivery.sender.clone(),
                    recipient: delivery.recipient.clone(),
                    payload: bytes.clone(),
                })
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::WouldBlock, "outbound worker is full")
                })?;
            Ok(Phase::Transport(delivery, bytes))
        }
        (
            Phase::Finalize(delivery, bytes, failure),
            Response::OutboundTransitioned { result: Ok(()), .. },
        ) => match failure {
            Some(failure) => {
                submit(
                    storage,
                    Request::AppendDsn {
                        request_id: next_id(ids)?,
                        reverse_path: delivery.sender,
                        recipient: delivery.recipient,
                        transaction_id: delivery.transaction_id,
                        original: bytes,
                        failure,
                        dsn_metadata: delivery.dsn_metadata.clone(),
                    },
                )?;
                Ok(Phase::Dsn(delivery.state))
            }
            None => next_job(storage, ids, remaining, report, delivery.state),
        },
        (Phase::Dsn(state), Response::DsnAppended { result: Ok(_), .. }) => {
            next_job(storage, ids, remaining, report, state)
        }
        (
            _,
            Response::OutboundDue {
                result: Err(error), ..
            },
        )
        | (
            _,
            Response::OutboundBlob {
                result: Err(error), ..
            },
        )
        | (
            _,
            Response::OutboundTransitioned {
                result: Err(error), ..
            },
        )
        | (
            _,
            Response::DsnAppended {
                result: Err(error), ..
            },
        ) => Err(error),
        _ => Err(io::Error::other(
            "unexpected storage completion for outbound service",
        )),
    }
}

fn handle_transport(
    storage: &WakingStorageWorker,
    phase: Phase,
    completion: crate::outbound_worker::Completion,
    ids: &mut u64,
) -> io::Result<Phase> {
    match phase {
        Phase::Transport(mut delivery, bytes) => {
            let state = outbound_policy::state(&completion.result, delivery.attempts);
            let failure = outbound_policy::requested_dsn_failure(
                &completion.result,
                delivery.attempts,
                delivery.dsn_metadata.as_deref(),
            );
            delivery.state = state;
            delivery.next_attempt_epoch =
                outbound_policy::next_attempt_epoch(state, delivery.attempts, now_epoch()?);
            submit(
                storage,
                Request::OutboundTransition {
                    request_id: next_id(ids)?,
                    delivery: delivery.clone(),
                    state,
                },
            )?;
            Ok(Phase::Finalize(delivery, bytes, failure))
        }
        _ => Err(io::Error::other("unexpected transport completion")),
    }
}

fn next_job(
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
