//! Неблокирующий TCP I/O одной session без управления poll или хранилищем.
use super::*;
use std::io::{Read, Write};

/// Выводит уже подготовленный wire-буфер и сообщает, нужно ли закрыть session.
pub(super) fn flush(session: &mut Option<Session>) -> io::Result<bool> {
    let Some(session) = session else {
        return Ok(true);
    };
    match session.stream.write(&session.output) {
        Ok(0) => Ok(true),
        Ok(count) => {
            session.output.drain(..count);
            Ok(false)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(false),
        Err(_) => Ok(true),
    }
}

/// Читает один ограниченный TCP-шаг и передаёт plaintext protocol FSM.
pub(super) fn read_session(
    session: &mut Option<Session>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    let Some(session) = session else {
        return Ok(true);
    };
    let mut bytes = [0_u8; READ_BUDGET];
    match session.stream.read(&mut bytes) {
        Ok(0) => return Ok(true),
        Ok(count) => match bytes.get(..count) {
            Some(received) => session.input.extend_from_slice(received),
            None => return Ok(true),
        },
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
        Err(_) => return Ok(true),
    }
    if session.input.len() > MAX_INPUT {
        return Ok(true);
    }
    process_input(session, slot, generation, worker, owners, request_id)?;
    Ok(false)
}
