//! Последовательность SMTP BDAT, проверка сообщения и передача на durable delivery.

use super::session::SmtpState;
use super::*;

/// Состояние письма сохраняется между промежуточными BDAT chunks.
pub(super) struct Transaction {
    pub(super) bytes: Vec<u8>,
    pub(super) reverse_path: String,
    pub(super) recipients: Vec<String>,
    pub(super) body_mode: smtp::BodyMode,
    pub(super) smtp_utf8: bool,
    pub(super) failed: bool,
}

impl Transaction {
    /// После первой ошибки освобождает message bytes и только дренирует chunks.
    pub(super) fn fail(&mut self) {
        self.bytes.clear();
        self.failed = true;
    }
}

/// Raw receiver вместе с выбранным marker-ом завершающего chunk.
pub(super) struct Chunk {
    pub(super) transaction: Transaction,
    pub(super) receiver: super::smtp_bdat_chunk::Receiver,
    pub(super) last: bool,
}

/// Принимает typed BDAT из обычного command state либо продолжает chunk sequence.
pub(super) fn start(
    session: &mut Session,
    command: smtp::BdatCommand,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let state = std::mem::replace(
        &mut session.smtp,
        SmtpState::Command(smtp::Transaction::default()),
    );
    match state {
        SmtpState::Command(envelope) => start_envelope(
            session, envelope, command, slot, generation, worker, owners, request_id,
        ),
        SmtpState::BdatReady(transaction) => start_next(
            session,
            transaction,
            command,
            slot,
            generation,
            worker,
            owners,
            request_id,
        ),
        state => {
            session.smtp = state;
            super::smtp_bdat_chunk::reject_sequence(session, command.chunk_size)
        }
    }
}

/// После ошибки разрешает только RSET и дренирует already-pipelined chunks.
pub(super) fn after_failure(session: &mut Session, line: &[u8]) -> io::Result<()> {
    match smtp::parse(std::str::from_utf8(line).unwrap_or_default()) {
        Ok(smtp::Command::Rset) => super::smtp_bdat_completion::reset(session),
        Ok(smtp::Command::Bdat(command)) => {
            session.smtp = SmtpState::BdatDiscard {
                receiver: super::smtp_bdat_chunk::Receiver::discard(command.chunk_size),
            };
            super::smtp_bdat_chunk::discard_received(session)
        }
        _ => session.push(b"503 5.5.1 failed BDAT requires RSET\r\n"),
    }
}

/// Внутри BDAT transaction разрешает только следующий BDAT либо обязательный RSET.
pub(super) fn between_chunks(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    match smtp::parse(std::str::from_utf8(&line).unwrap_or_default()) {
        Ok(smtp::Command::Bdat(command)) => start(
            session, command, slot, generation, worker, owners, request_id,
        ),
        Ok(smtp::Command::Rset) => super::smtp_bdat_completion::reset(session),
        _ => fail_sequence(session),
    }
}

/// Отмечает транзакцию ошибочной, поскольку BDAT sequence нельзя продолжить.
fn fail_sequence(session: &mut Session) -> io::Result<()> {
    let previous = std::mem::replace(
        &mut session.smtp,
        SmtpState::Command(smtp::Transaction::default()),
    );
    match previous {
        SmtpState::BdatReady(mut transaction) => {
            transaction.fail();
            session.smtp = SmtpState::BdatReady(transaction);
            session.push(b"503 5.5.1 BDAT sequence requires BDAT or RSET\r\n")
        }
        state => {
            session.smtp = state;
            session.push(b"503 5.5.1 bad sequence\\r\\n")
        }
    }
}

/// Получает очередную часть raw input, не разбирая её как CRLF-команды.
pub(super) fn receive(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<bool> {
    let progress = match &mut session.smtp {
        SmtpState::BdatReceiving(chunk) => chunk
            .receiver
            .consume(&mut session.input, &mut chunk.transaction.bytes),
        _ => return Ok(true),
    };
    match progress {
        super::smtp_bdat_chunk::Progress::Waiting => Ok(false),
        super::smtp_bdat_chunk::Progress::Complete => super::smtp_bdat_completion::finish_chunk(
            session, false, slot, generation, worker, owners, request_id,
        ),
        super::smtp_bdat_chunk::Progress::TooLarge => super::smtp_bdat_completion::finish_chunk(
            session, true, slot, generation, worker, owners, request_id,
        ),
    }
}

/// Переводит валидный MAIL/RCPT envelope в режим приёма BDAT.
#[allow(clippy::too_many_arguments)]
fn start_envelope(
    session: &mut Session,
    mut envelope: smtp::Transaction,
    command: smtp::BdatCommand,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if !envelope.can_start_data() {
        session.smtp = SmtpState::Command(envelope);
        return super::smtp_bdat_chunk::reject_sequence(session, command.chunk_size);
    }
    let envelope = envelope.take_envelope();
    let transaction = Transaction {
        bytes: Vec::new(),
        reverse_path: envelope.reverse_path,
        recipients: envelope.recipients,
        body_mode: session.smtp_body_mode,
        smtp_utf8: session.smtp_utf8,
        failed: false,
    };
    begin_chunk(
        session,
        transaction,
        command,
        slot,
        generation,
        worker,
        owners,
        request_id,
    )
}

/// Добавляет следующий chunk или только дренирует его после прежнего отказа.
#[allow(clippy::too_many_arguments)]
fn start_next(
    session: &mut Session,
    transaction: Transaction,
    command: smtp::BdatCommand,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let receiver = match transaction.failed {
        true => super::smtp_bdat_chunk::Receiver::discard(command.chunk_size),
        false => super::smtp_bdat_chunk::Receiver::new(
            command.chunk_size,
            transaction.bytes.len(),
            MAX_MESSAGE,
        ),
    };
    begin_chunk_with_receiver(
        session,
        transaction,
        receiver,
        command.last,
        slot,
        generation,
        worker,
        owners,
        request_id,
    )
}

/// Регистрирует chunk и сразу завершает нулевой literal без чтения сокета.
#[allow(clippy::too_many_arguments)]
fn begin_chunk(
    session: &mut Session,
    transaction: Transaction,
    command: smtp::BdatCommand,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let receiver = super::smtp_bdat_chunk::Receiver::new(
        command.chunk_size,
        transaction.bytes.len(),
        MAX_MESSAGE,
    );
    begin_chunk_with_receiver(
        session,
        transaction,
        receiver,
        command.last,
        slot,
        generation,
        worker,
        owners,
        request_id,
    )
}

/// Сохраняет session owner до получения полного заявленного octet count.
#[allow(clippy::too_many_arguments)]
fn begin_chunk_with_receiver(
    session: &mut Session,
    transaction: Transaction,
    receiver: super::smtp_bdat_chunk::Receiver,
    last: bool,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    session.smtp = SmtpState::BdatReceiving(Chunk {
        transaction,
        receiver,
        last,
    });
    match receive(session, slot, generation, worker, owners, request_id) {
        Ok(true) => Ok(()),
        Ok(false) => Ok(()),
        Err(error) => Err(error),
    }
}
