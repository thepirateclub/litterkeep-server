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
#[path = "outbound_service_dsn.rs"]
mod dsn_request;
#[path = "outbound_service_progress.rs"]
mod progress;

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
pub(super) enum Phase {
    Due,
    Blob(OwnedDelivery),
    Start(OwnedDelivery, Vec<u8>),
    Transport(OwnedDelivery, Vec<u8>),
    Finalize(OwnedDelivery, Vec<u8>, Option<dsn::Notice>),
    Dsn(OwnedDelivery),
    Acknowledge(OwnedDelivery),
    Finished,
}

fn readable() -> Interest {
    Interest {
        readable: true,
        writable: false,
    }
}

pub(super) fn request_due(storage: &WakingStorageWorker, ids: &mut u64) -> io::Result<()> {
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
            Phase::Blob(delivery),
            Response::OutboundBlob {
                result: Ok(bytes), ..
            },
        ) if let Some(notice) = delivery.dsn_notice => {
            dsn_request::submit(storage, ids, delivery, bytes, notice)
        }
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
                    dsn_metadata: delivery.dsn_metadata.as_deref().cloned(),
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
            Some(failure) => dsn_request::submit(storage, ids, delivery, bytes, failure),
            None => progress::next_job(storage, ids, remaining, report, delivery.state),
        },
        (
            Phase::Dsn(mut delivery),
            Response::Dsn(crate::storage_worker::DsnResponse::Report { result: Ok(_), .. }),
        ) => {
            delivery.dsn_notice = None;
            delivery.dsn_diagnostic = None;
            let state = delivery.state;
            submit(
                storage,
                Request::OutboundTransition {
                    request_id: next_id(ids)?,
                    delivery: delivery.clone(),
                    state,
                },
            )?;
            Ok(Phase::Acknowledge(delivery))
        }
        (Phase::Acknowledge(delivery), Response::OutboundTransitioned { result: Ok(()), .. }) => {
            progress::next_job(storage, ids, remaining, report, delivery.state)
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
            Response::Dsn(crate::storage_worker::DsnResponse::Report {
                result: Err(error), ..
            }),
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
            let notice = outbound_policy::requested_dsn_notice(
                &completion.result,
                delivery.attempts,
                delivery.dsn_metadata.as_deref(),
            );
            delivery.state = state;
            delivery.dsn_notice = notice;
            delivery.dsn_diagnostic = match notice {
                Some(dsn::Notice::Failure(_)) => completion.diagnostic,
                None => None,
                Some(dsn::Notice::Relayed) => None,
            };
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
            Ok(Phase::Finalize(delivery, bytes, notice))
        }
        _ => Err(io::Error::other("unexpected transport completion")),
    }
}
