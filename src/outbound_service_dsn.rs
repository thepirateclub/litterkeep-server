//! Формирование и постановка идемпотентного failure DSN в Store owner.

use crate::{
    dsn,
    outbound_service::Phase,
    outbound_service_support::{next_id, submit as submit_request},
    queue::OwnedDelivery,
    storage_worker::{Request, WakingStorageWorker},
};
use std::io;

/// Передаёт report request с ключом, сохраняемым Store до подтверждения очереди.
pub(super) fn submit(
    storage: &WakingStorageWorker,
    ids: &mut u64,
    delivery: OwnedDelivery,
    bytes: Vec<u8>,
    failure: dsn::Failure,
) -> io::Result<Phase> {
    let report_key = delivery
        .dsn_report_key()
        .ok_or_else(|| io::Error::other("terminal DSN notice has no idempotency key"))?;
    submit_request(
        storage,
        Request::AppendDsn {
            request_id: next_id(ids)?,
            reverse_path: delivery.sender.clone(),
            recipient: delivery.recipient.clone(),
            transaction_id: delivery.transaction_id,
            original: bytes,
            failure,
            dsn_metadata: delivery.dsn_metadata.clone(),
            report_key,
        },
    )?;
    Ok(Phase::Dsn(delivery))
}
