//! Короткие helpers bounded outbound service state machine.

use crate::storage_worker::{Request, WakingStorageWorker};
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// Передаёт request единственному Store owner без блокировки event loop.
pub(crate) fn submit(storage: &WakingStorageWorker, request: Request) -> io::Result<()> {
    storage.worker.submit(request).map_err(|error| match error {
        crate::storage_worker::SubmitError::Full(_) => {
            io::Error::new(io::ErrorKind::WouldBlock, "storage worker is full")
        }
        crate::storage_worker::SubmitError::Stopped => {
            io::Error::new(io::ErrorKind::BrokenPipe, "storage worker stopped")
        }
    })
}

/// Выдаёт monotonic service-local request ID без silent wraparound.
pub(crate) fn next_id(ids: &mut u64) -> io::Result<u64> {
    let value = *ids;
    *ids = ids
        .checked_add(1)
        .ok_or_else(|| io::Error::other("outbound request id exhausted"))?;
    Ok(value)
}

/// Получает wall clock только для durable queue deadline, не reactor interval.
pub(crate) fn now_epoch() -> io::Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(|_| io::Error::other("system clock predates Unix epoch"))
}
