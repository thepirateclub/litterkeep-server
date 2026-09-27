//! Завершение SMTP DATA после dot terminator.
use super::*;

/// Окончательно принимает DATA либо отвергает её после dot terminator.
pub(super) fn finish_data(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
    failure: Option<DataFailure>,
) -> io::Result<()> {
    match failure {
        Some(DataFailure::MessageTooLarge) => {
            reject_data(session, b"552 5.3.4 message too large\r\n")
        }
        Some(DataFailure::InvalidOctets) => reject_data(
            session,
            b"554 5.6.0 NUL is not allowed in 7BIT or 8BITMIME DATA\r\n",
        ),
        None => accept_data(session, slot, generation, worker, owners, request_id),
    }
}

/// Возвращает отвергнутую DATA-транзакцию в command state.
fn reject_data(session: &mut Session, response: &[u8]) -> io::Result<()> {
    reset_data_state(session);
    session.push(response)
}

/// Сохраняет допустимое DATA-сообщение через владельца долговечного Store.
fn accept_data(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let (bytes, reverse_path, recipients) = super::take_data(session);
    reset_data_state(session);
    let message = crate::smtp_trace::prepend(&bytes, &reverse_path, std::time::SystemTime::now())?;
    super::submit_delivery(
        session,
        super::Delivery {
            bytes: message,
            reverse_path,
            recipients,
        },
        slot,
        generation,
        worker,
        owners,
        request_id,
    )
}

/// Сбрасывает MAIL extension markers после завершения или отказа DATA.
fn reset_data_state(session: &mut Session) {
    session.smtp = SmtpState::Command(smtp::Transaction::default());
    session.smtp_body_mode = smtp::BodyMode::SevenBit;
    session.smtp_utf8 = false;
}
