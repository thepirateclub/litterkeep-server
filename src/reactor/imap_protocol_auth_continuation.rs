//! Обработка отдельной строки продолжения IMAP SASL exchange.

use super::imap_protocol::authenticate;
use super::{Pending, Session, WakingStorageWorker};
use std::collections::HashMap;
use std::io;

/// Продолжает SASL exchange, уже отделённый от обычной командной grammar.
pub(in crate::reactor) fn handle(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    authenticate::continuation(session, &line, slot, generation, worker, owners, request_id)
}
