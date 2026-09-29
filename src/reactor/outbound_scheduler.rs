//! Долгоживущий владелец исходящей очереди внутри общего reactor-а.

use super::{Orchestrator, Response, Session, scheduler_ids::SchedulerIds, scheduler_lifecycle};
use crate::runtime::{Event, TaskId};
use crate::{
    dsn, outbound_policy,
    outbound_service_support::{now_epoch, submit},
    outbound_wakeup::WakingOutboundWorker,
    queue::{OwnedDelivery, State},
    storage_worker::{Request, WakingStorageWorker},
};
use std::{
    io,
    time::{Duration, Instant},
};
#[path = "outbound_scheduler_dsn.rs"]
mod dsn_request;

/// Интервал повторного просмотра durable deadline-index после пустой выборки.
const IDLE_SCAN: Duration = Duration::from_secs(1);

/// Единственный scheduler владеет одной delivery от чтения blob до фиксации итога.
pub(super) struct OutboundScheduler {
    pub(super) outbound: WakingOutboundWorker,
    pub(super) ids: SchedulerIds,
    pub(super) task: TaskId,
    pub(super) phase: Phase,
}

pub(super) enum Phase {
    Due(u64),
    Blob(u64, OwnedDelivery),
    Start(u64, OwnedDelivery, Vec<u8>),
    Transport(u64, OwnedDelivery, Vec<u8>),
    Finalize(u64, OwnedDelivery, Vec<u8>, Option<dsn::Failure>),
    Dsn(u64, OwnedDelivery),
    AcknowledgeDsn(u64),
    Idle(Instant),
}

impl OutboundScheduler {
    /// Запускает bounded transport owner и сразу запрашивает готовую delivery.
    pub(super) fn start(
        route: crate::outbound_route::Route,
        storage: &WakingStorageWorker,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<Self> {
        let task = lifecycle.spawn_task();
        let mut scheduler = Self {
            outbound: WakingOutboundWorker::start_route(route, 1, 2)?,
            ids: SchedulerIds::new(),
            task,
            phase: Phase::Idle(Instant::now()),
        };
        scheduler.request_due(storage, lifecycle)?;
        Ok(scheduler)
    }

    pub(super) fn wake_reader(&self) -> &std::os::unix::net::UnixStream {
        self.outbound.wake_reader()
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Idle(deadline) => Some(deadline),
            _ => None,
        }
    }

    pub(super) fn wake_due(
        &mut self,
        storage: &WakingStorageWorker,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<bool> {
        if let (true, true) = (
            scheduler_lifecycle::cancelling(lifecycle, self.task),
            matches!(self.phase, Phase::Idle(_)),
        ) {
            return Ok(true);
        }
        match self.phase {
            Phase::Idle(deadline) if Instant::now() >= deadline => {
                scheduler_lifecycle::wake(lifecycle, self.task, Event::Timer(self.task))?;
                self.request_due(storage, lifecycle)?;
            }
            _ => {}
        }
        Ok(false)
    }

    pub(super) fn storage_completed(
        &mut self,
        storage: &WakingStorageWorker,
        response: Response,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<()> {
        let response_id = super::completion_id::response_id(&response);
        match self.expected_storage_id() {
            Some(expected) if expected == response_id => {}
            _ => return Err(io::Error::other("unowned scheduler storage completion")),
        }
        scheduler_lifecycle::wake(lifecycle, self.task, Event::Completion(response_id))?;
        self.phase = self.next_storage_phase(storage, response, lifecycle)?;
        Ok(())
    }

    /// Вычитывает authoritative completion channel после coalesced wakeup.
    pub(super) fn transport_completed(
        &mut self,
        storage: &WakingStorageWorker,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<()> {
        self.outbound.drain_wakeup()?;
        while let Ok(completion) = self.outbound.worker.try_completion() {
            scheduler_lifecycle::wake(lifecycle, self.task, Event::Completion(completion.id))?;
            let phase = std::mem::replace(&mut self.phase, Phase::Idle(Instant::now()));
            self.phase = match phase {
                Phase::Transport(job_id, mut delivery, bytes) if job_id == completion.id => {
                    let state = outbound_policy::state(&completion.result, delivery.attempts);
                    let failure = outbound_policy::requested_dsn_failure(
                        &completion.result,
                        delivery.attempts,
                        delivery.dsn_metadata.as_deref(),
                    );
                    delivery.state = state;
                    delivery.dsn_failure = failure;
                    delivery.dsn_diagnostic = match failure {
                        Some(_) => completion.diagnostic,
                        None => None,
                    };
                    delivery.next_attempt_epoch =
                        outbound_policy::next_attempt_epoch(state, delivery.attempts, now_epoch()?);
                    let id = self.submit_transition(storage, &delivery, state, lifecycle)?;
                    Phase::Finalize(id, delivery, bytes, failure)
                }
                _ => return Err(io::Error::other("unowned outbound completion")),
            };
        }
        Ok(())
    }

    fn next_storage_phase(
        &mut self,
        storage: &WakingStorageWorker,
        response: Response,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<Phase> {
        let phase = std::mem::replace(&mut self.phase, Phase::Idle(Instant::now()));
        match (phase, response) {
            (
                Phase::Due(_),
                Response::OutboundDue {
                    result: Ok(mut jobs),
                    ..
                },
            ) => {
                if scheduler_lifecycle::cancelling(lifecycle, self.task) {
                    return Ok(Phase::Idle(Instant::now()));
                }
                let Some(delivery) = jobs.pop() else {
                    scheduler_lifecycle::wait(lifecycle, self.task, Event::Timer(self.task))?;
                    return Ok(Phase::Idle(Instant::now() + IDLE_SCAN));
                };
                let id = self.take_id()?;
                submit(
                    storage,
                    Request::OutboundBlob {
                        request_id: id,
                        delivery: delivery.clone(),
                    },
                )?;
                scheduler_lifecycle::wait(lifecycle, self.task, Event::Completion(id))?;
                Ok(Phase::Blob(id, delivery))
            }
            (
                Phase::Blob(_, delivery),
                Response::OutboundBlob {
                    result: Ok(bytes), ..
                },
            ) if let Some(failure) = delivery.dsn_failure => {
                self.submit_dsn(storage, delivery, bytes, failure, lifecycle)
            }
            (
                Phase::Blob(_, mut delivery),
                Response::OutboundBlob {
                    result: Ok(bytes), ..
                },
            ) => {
                delivery.attempts = delivery
                    .attempts
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("outbound attempts exhausted"))?;
                delivery.state = State::InFlight;
                let id = self.submit_transition(storage, &delivery, State::InFlight, lifecycle)?;
                Ok(Phase::Start(id, delivery, bytes))
            }
            (
                Phase::Start(_, delivery, bytes),
                Response::OutboundTransitioned { result: Ok(()), .. },
            ) => {
                let job_id = self.take_id()?;
                self.outbound
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
                scheduler_lifecycle::wait(lifecycle, self.task, Event::Completion(job_id))?;
                Ok(Phase::Transport(job_id, delivery, bytes))
            }
            (
                Phase::Finalize(_, delivery, bytes, Some(failure)),
                Response::OutboundTransitioned { result: Ok(()), .. },
            ) => self.submit_dsn(storage, delivery, bytes, failure, lifecycle),
            (
                Phase::Finalize(_, _, _, None),
                Response::OutboundTransitioned { result: Ok(()), .. },
            )
            | (Phase::AcknowledgeDsn(_), Response::OutboundTransitioned { result: Ok(()), .. }) => {
                self.continue_or_stop(storage, lifecycle)
            }
            (
                Phase::Dsn(_, mut delivery),
                Response::Dsn(crate::storage_worker::DsnResponse::Failure {
                    result: Ok(_), ..
                }),
            ) => {
                delivery.dsn_failure = None;
                delivery.dsn_diagnostic = None;
                let state = delivery.state;
                let id = self.submit_transition(storage, &delivery, state, lifecycle)?;
                Ok(Phase::AcknowledgeDsn(id))
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
                Response::Dsn(crate::storage_worker::DsnResponse::Failure {
                    result: Err(error),
                    ..
                }),
            ) => Err(error),
            _ => Err(io::Error::other("unexpected scheduler storage completion")),
        }
    }

    fn expected_storage_id(&self) -> Option<u64> {
        match self.phase {
            Phase::Due(id)
            | Phase::Blob(id, _)
            | Phase::Start(id, _, _)
            | Phase::Finalize(id, _, _, _)
            | Phase::Dsn(id, _)
            | Phase::AcknowledgeDsn(id) => Some(id),
            Phase::Transport(_, _, _) | Phase::Idle(_) => None,
        }
    }

    fn continue_or_stop(
        &mut self,
        storage: &WakingStorageWorker,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<Phase> {
        if scheduler_lifecycle::cancelling(lifecycle, self.task) {
            return Ok(Phase::Idle(Instant::now()));
        }
        self.request_due(storage, lifecycle)?;
        Ok(std::mem::replace(
            &mut self.phase,
            Phase::Idle(Instant::now()),
        ))
    }
}
