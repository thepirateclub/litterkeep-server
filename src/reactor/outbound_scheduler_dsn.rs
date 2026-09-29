//! Публикация recovery-safe DSN notice из scheduler-а.

use super::{Orchestrator, OutboundScheduler, Phase, Session, scheduler_lifecycle};
use crate::runtime::Event;
use crate::{
    dsn,
    outbound_service_support::submit,
    queue::OwnedDelivery,
    storage_worker::{DsnRequest, Request, WakingStorageWorker},
};
use std::io;

impl OutboundScheduler {
    /// Публикует сохранившееся notice, не меняя число transport attempts.
    pub(super) fn submit_dsn(
        &mut self,
        storage: &WakingStorageWorker,
        delivery: OwnedDelivery,
        bytes: Vec<u8>,
        notice: dsn::Notice,
        lifecycle: &mut Orchestrator<Session>,
    ) -> io::Result<Phase> {
        let report_key = delivery
            .dsn_report_key()
            .ok_or_else(|| io::Error::other("terminal DSN notice has no idempotency key"))?;
        let id = self
            .ids
            .take()
            .ok_or_else(|| io::Error::other("outbound scheduler request IDs exhausted"))?;
        submit(
            storage,
            Request::Dsn(DsnRequest::Report {
                request_id: id,
                reverse_path: delivery.sender.clone(),
                recipient: delivery.recipient.clone(),
                transaction_id: delivery.transaction_id,
                original: bytes,
                notice,
                dsn_metadata: delivery.dsn_metadata.clone(),
                diagnostic: delivery.dsn_diagnostic.clone(),
                report_key,
            }),
        )?;
        scheduler_lifecycle::wait(lifecycle, self.task, Event::Completion(id))?;
        Ok(Phase::Dsn(id, delivery))
    }
}
