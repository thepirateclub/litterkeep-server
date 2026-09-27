//! Ограниченный приём заданного числа SMTP BDAT октетов.

use super::Session;
use super::session::SmtpState;

/// Результат очередного чтения из session input buffer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Progress {
    /// Для завершения заявленного chunk нужны следующие TCP octets.
    Waiting,
    /// Все октеты chunk получены и сохранены в сообщении.
    Complete,
    /// Chunk полностью прочитан, но не помещается в остаток message budget.
    TooLarge,
}

/// Состояние точного octet-counted framing одной BDAT команды.
pub(super) struct Receiver {
    remaining: u64,
    oversized: bool,
}

impl Receiver {
    /// Создаёт receiver и заранее выбирает drain-only режим сверх лимита.
    pub(super) fn new(chunk_size: u64, message_size: usize, message_limit: usize) -> Self {
        let available = message_limit.saturating_sub(message_size);
        let oversized = usize::try_from(chunk_size).map_or(true, |size| size > available);
        Self {
            remaining: chunk_size,
            oversized,
        }
    }

    /// Поглощает только заявленные octets и сохраняет за границей следующие команды.
    pub(super) fn consume(&mut self, input: &mut Vec<u8>, message: &mut Vec<u8>) -> Progress {
        let consumed = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(input.len());
        if !self.oversized {
            message.extend(input.drain(..consumed));
        } else {
            input.drain(..consumed);
        }
        self.remaining = self.remaining.saturating_sub(consumed as u64);
        match (self.remaining, self.oversized) {
            (0, false) => Progress::Complete,
            (0, true) => Progress::TooLarge,
            (_, _) => Progress::Waiting,
        }
    }
}

/// Точно дренирует тело недоговорённого BDAT, не интерпретируя его как команды.
pub(super) fn reject_unadvertised(session: &mut Session, chunk_size: u64) -> std::io::Result<()> {
    let previous = std::mem::replace(
        &mut session.smtp,
        SmtpState::Command(crate::smtp::Transaction::default()),
    );
    match previous {
        SmtpState::Command(transaction) => {
            session.smtp = SmtpState::BdatDiscard {
                transaction,
                receiver: Receiver::new(chunk_size, 0, 0),
            };
            discard_received(session)
        }
        state => {
            session.smtp = state;
            session.push(b"503 5.5.1 bad sequence\r\n")
        }
    }
}

/// Обрабатывает очередной raw read и после точного boundary возвращает 502.
pub(super) fn discard_received(session: &mut Session) -> std::io::Result<()> {
    let progress = match &mut session.smtp {
        SmtpState::BdatDiscard { receiver, .. } => {
            receiver.consume(&mut session.input, &mut Vec::new())
        }
        _ => return Ok(()),
    };
    match progress {
        Progress::Waiting => Ok(()),
        Progress::Complete | Progress::TooLarge => {
            let previous = std::mem::replace(
                &mut session.smtp,
                SmtpState::Command(crate::smtp::Transaction::default()),
            );
            match previous {
                SmtpState::BdatDiscard { transaction, .. } => {
                    session.smtp = SmtpState::Command(transaction);
                    session.push(b"502 5.5.1 CHUNKING not supported\r\n")
                }
                state => {
                    session.smtp = state;
                    Ok(())
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "smtp_bdat_chunk_tests.rs"]
mod tests;
