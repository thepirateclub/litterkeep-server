//! Повторная постановка bounded requests после освобождения worker capacity.

use super::*;

pub(super) fn retry_submissions(
    worker: &mut WakingStorageWorker,
    sessions: &mut [Option<SessionLease>],
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
) -> io::Result<()> {
    for (slot, entry) in sessions.iter_mut().enumerate() {
        let Some(lease) = entry else { continue };
        let session = lease.get_mut();
        let Some((request, pending)) = session.pending_submit.take() else {
            continue;
        };
        protocol::submit(
            session,
            request,
            pending,
            slot,
            session.generation,
            worker,
            owners,
        );
    }
    Ok(())
}
