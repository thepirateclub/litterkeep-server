//! Завершение BDAT chunks, проверка сообщения и долговечная передача в Store.

use super::session::{DataSection, SmtpState};
use super::smtp_bdat::{Chunk, Transaction};
use super::*;

/// Подтверждает принятый чанк или сохраняет failed transaction до RSET.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_chunk(
    session: &mut Session,
    too_large: bool,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    let previous = std::mem::replace(
        &mut session.smtp,
        SmtpState::Command(smtp::Transaction::default()),
    );
    let SmtpState::BdatReceiving(mut chunk) = previous else {
        return Ok(true);
    };
    match (chunk.transaction.failed, too_large) {
        (true, _) => {
            session.smtp = SmtpState::BdatReady(chunk.transaction);
            session.push(b"503 5.5.1 BDAT transaction failed; send RSET\r\n")?;
        }
        (false, true) => {
            chunk.transaction.fail();
            session.smtp = SmtpState::BdatReady(chunk.transaction);
            session.push(b"552 5.3.4 message size exceeds fixed maximum\r\n")?;
        }
        (false, false) => {
            return finish_accepted_chunk(
                session, chunk, slot, generation, worker, owners, request_id,
            );
        }
    }
    Ok(true)
}

/// Оставляет промежуточный chunk открытым или завершает durable delivery на LAST.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_accepted_chunk(
    session: &mut Session,
    chunk: Chunk,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    if !chunk.last {
        session.smtp = SmtpState::BdatReady(chunk.transaction);
        session.push(b"250 2.0.0 BDAT chunk accepted\r\n")?;
        return Ok(true);
    }
    if !valid_message(&chunk.transaction) {
        let mut transaction = chunk.transaction;
        transaction.fail();
        session.smtp = SmtpState::BdatReady(transaction);
        session.push(b"552 5.6.0 message content violates negotiated SMTP body profile\r\n")?;
        return Ok(true);
    }
    let transaction = chunk.transaction;
    session.smtp = SmtpState::Command(smtp::Transaction::default());
    session.smtp_body_mode = smtp::BodyMode::SevenBit;
    session.smtp_utf8 = false;
    super::smtp_data::submit_bdat(
        session,
        super::smtp_data::Delivery {
            bytes: transaction.bytes,
            reverse_path: transaction.reverse_path,
            recipients: transaction.recipients,
        },
        slot,
        generation,
        worker,
        owners,
        request_id,
    )?;
    Ok(true)
}

/// Проверяет canonical MIME и объявленный 7/8-bit либо binary body profile.
fn valid_message(transaction: &Transaction) -> bool {
    match transaction.body_mode {
        smtp::BodyMode::BinaryMime => {
            return crate::outbound::validate_bdat_binary(
                &transaction.bytes,
                transaction.smtp_utf8,
            );
        }
        smtp::BodyMode::SevenBit | smtp::BodyMode::EightBitMime => {}
    }
    let mut section = DataSection::Headers;
    let mut remaining = transaction.bytes.as_slice();
    while !remaining.is_empty() {
        let Some(end) = remaining.windows(2).position(|pair| pair == b"\r\n") else {
            return false;
        };
        let line_end = end.saturating_add(2);
        let Some(line) = remaining.get(..line_end) else {
            return false;
        };
        let Some(content) = line.strip_suffix(b"\r\n") else {
            return false;
        };
        let invalid_octets = content
            .iter()
            .any(|octet| matches!(*octet, 0 | b'\r' | b'\n'));
        let headers = matches!(section, DataSection::Headers);
        if invalid_octets
            || super::smtp_data::data_line_accepts(
                transaction.body_mode,
                transaction.smtp_utf8,
                headers,
                line,
            )
            .is_err()
        {
            return false;
        }
        super::smtp_data::advance_section(&mut section, line);
        let Some(tail) = remaining.get(line_end..) else {
            return false;
        };
        remaining = tail;
    }
    true
}

/// Сбрасывает failed transaction и возвращает SMTP-сессию в command state.
pub(super) fn reset(session: &mut Session) -> io::Result<()> {
    session.smtp = SmtpState::Command(smtp::Transaction::default());
    session.smtp_body_mode = smtp::BodyMode::SevenBit;
    session.smtp_utf8 = false;
    session.push(b"250 2.0.0 reset\r\n")
}

#[cfg(test)]
#[path = "smtp_bdat_completion_tests.rs"]
mod tests;
