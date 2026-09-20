//! Единый путь освобождения session lease и связанного POP3 maildrop lease.

use super::{MaildropLeases, Orchestrator, Session, SessionLease};
use std::io;

pub(super) fn close(
    entry: &mut Option<SessionLease>,
    lifecycle: &mut Orchestrator<Session>,
    maildrops: &mut MaildropLeases,
) -> io::Result<()> {
    let Some(lease) = entry.take() else {
        return Ok(());
    };
    maildrops.release_generation(lease.get().generation);
    lifecycle
        .close(lease)
        .map_err(|error| io::Error::other(format!("session close failed: {error:?}")))
}
