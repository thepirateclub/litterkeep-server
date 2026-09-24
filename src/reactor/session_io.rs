//! Неблокирующий TCP I/O одной session без управления poll или хранилищем.
use super::*;
use std::io::{Read, Write};

/// Выводит один ограниченный TCP-шаг и сообщает, нужно ли закрыть session.
pub(super) fn flush(session: &mut Option<SessionLease>) -> io::Result<bool> {
    let Some(lease) = session else {
        return Ok(true);
    };
    let session = lease.get_mut();
    if prepare_wire_output(session).is_err() {
        return Ok(true);
    }
    let output = match session.tls {
        Some(_) => &mut session.wire_output,
        None => &mut session.output,
    };
    match session.stream.write(output) {
        Ok(0) => Ok(true),
        Ok(count) => {
            output.drain(..count);
            session.begin_starttls_if_ready()?;
            Ok(false)
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(false),
        Err(_) => Ok(true),
    }
}

/// Читает TCP bytes, при необходимости расшифровывает их и передаёт protocol FSM.
pub(super) fn read_session(
    session: &mut Option<SessionLease>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    let Some(lease) = session else {
        return Ok(true);
    };
    let session = lease.get_mut();
    let mut bytes = [0_u8; READ_BUDGET];
    match session.stream.read(&mut bytes) {
        Ok(0) => return Ok(true),
        Ok(count) => match bytes.get(..count) {
            Some(received) => match receive_plaintext(session, received) {
                Ok(plaintext) => session.input.extend_from_slice(&plaintext),
                Err(_) => return Ok(true),
            },
            None => return Ok(true),
        },
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
        Err(_) => return Ok(true),
    }
    if session.input_limit_exceeded() {
        return Ok(true);
    }
    if session.tls.as_ref().is_some_and(ServerTls::is_handshaking) {
        return Ok(false);
    }
    process_input(session, slot, generation, worker, owners, request_id)?;
    Ok(false)
}

/// Переносит накопленные protocol bytes в TLS и забирает records для TCP.
fn prepare_wire_output(session: &mut Session) -> io::Result<()> {
    let Some(tls) = session.tls.as_mut() else {
        return Ok(());
    };
    if let (false, false) = (tls.is_handshaking(), session.output.is_empty()) {
        let accepted = tls.queue_plaintext(&session.output)?;
        session.output.drain(..accepted);
    }
    let encrypted = tls.take_encrypted()?;
    let next_size = session.wire_output.len().saturating_add(encrypted.len());
    if next_size > MAX_OUTPUT {
        return Err(io::Error::other("TLS output limit"));
    }
    session.wire_output.extend_from_slice(&encrypted);
    Ok(())
}

/// Расшифровывает bytes и откладывает greeting до завершения handshake.
fn receive_plaintext(session: &mut Session, received: &[u8]) -> io::Result<Vec<u8>> {
    let Some(tls) = session.tls.as_mut() else {
        return Ok(received.to_vec());
    };
    let plaintext = tls.receive(received)?;
    session.send_greeting_if_ready()?;
    Ok(plaintext)
}
