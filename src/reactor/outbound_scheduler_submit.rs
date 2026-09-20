//! Отправка Store-команд и завершение transport owner-а scheduler-а.

use super::{
    Orchestrator, Session,
    outbound_scheduler::{OutboundScheduler, Phase},
};
use crate::{
    outbound_service_support::{now_epoch, submit},
    queue::{OwnedDelivery, State},
    runtime::Event,
    storage_worker::{Request, WakingStorageWorker},
};
use std::io;

impl OutboundScheduler {
    pub(super) fn request_due(
        &mut self,
        storage: &WakingStorageWorker,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<()> {
        let id = self.take_id()?;
        submit(
            storage,
            Request::OutboundDue {
                request_id: id,
                now_epoch: now_epoch()?,
                limit: 1,
            },
        )?;
        super::scheduler_lifecycle::wait(lifecycle, self.task, Event::Completion(id))?;
        self.phase = Phase::Due(id);
        Ok(())
    }

    pub(super) fn submit_transition(
        &mut self,
        storage: &WakingStorageWorker,
        delivery: &OwnedDelivery,
        state: State,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<u64> {
        let id = self.take_id()?;
        submit(
            storage,
            Request::OutboundTransition {
                request_id: id,
                delivery: delivery.clone(),
                state,
            },
        )?;
        super::scheduler_lifecycle::wait(lifecycle, self.task, Event::Completion(id))?;
        Ok(id)
    }

    pub(super) fn take_id(&mut self) -> io::Result<u64> {
        self.ids
            .take()
            .ok_or_else(|| io::Error::other("scheduler request id exhausted"))
    }

    /// Закрывает job channel, присоединяет transport thread и удаляет task record.
    pub(super) fn shutdown(self, lifecycle: &mut Orchestrator<Session>) -> io::Result<()> {
        self.outbound.shutdown();
        match lifecycle.retire_task(self.task) {
            true => Ok(()),
            false => Err(io::Error::other("scheduler task still owns resources")),
        }
    }
}
