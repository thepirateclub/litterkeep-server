//! SMTP DATA octet handling and durable local-delivery handoff.
use super::*;
use crate::local_route::local_mailboxes;

/// Завершённое MIME-сообщение с SMTP envelope для bounded Store request.
struct Delivery {
    bytes: Vec<u8>,
    reverse_path: String,
    recipients: Vec<String>,
}

/// Принимает одну DATA-строку, ограничивая память и сохраняя границу команды.
pub(super) fn data_line(
    session: &mut Session,
    line: Vec<u8>,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    if line.as_slice() == b".\r\n" {
        let oversized = match &session.smtp {
            SmtpState::Data { oversized, .. } => *oversized,
            SmtpState::Command(_) => false,
        };
        return finish_data(
            session, slot, generation, worker, owners, request_id, oversized,
        );
    }
    let SmtpState::Data {
        bytes,
        oversized,
        body_mode,
        ..
    } = &mut session.smtp
    else {
        return Ok(());
    };
    match *oversized {
        true => Ok(()),
        false => append_data_line(bytes, oversized, *body_mode, &line),
    }
}

/// Окончательно принимает DATA либо отвергает её после dot terminator.
fn finish_data(
    session: &mut Session,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
    oversized: bool,
) -> io::Result<()> {
    if oversized {
        session.smtp = SmtpState::Command(smtp::Transaction::default());
        session.smtp_body_mode = smtp::BodyMode::SevenBit;
        return session.push(b"552 5.3.4 message too large\r\n");
    }
    let (bytes, reverse_path, recipients) = take_data(session);
    session.smtp = SmtpState::Command(smtp::Transaction::default());
    session.smtp_body_mode = smtp::BodyMode::SevenBit;
    let message = crate::smtp_trace::prepend(&bytes, &reverse_path, std::time::SystemTime::now())?;
    submit_delivery(
        session,
        Delivery {
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

/// Передаёт принятое сообщение единственному владельцу durable Store.
fn submit_delivery(
    session: &mut Session,
    delivery: Delivery,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    let envelope_recipients = delivery.recipients;
    let mailboxes = local_mailboxes(envelope_recipients.clone());
    let id = next_request(request_id);
    let pending = Pending::Append {
        mailboxes: mailboxes.clone(),
    };
    super::protocol::submit(
        session,
        Request::AppendMany {
            request_id: id,
            mailboxes,
            envelope_recipients,
            envelope_sender: delivery.reverse_path,
            bytes: delivery.bytes,
        },
        pending,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Переносит поля завершённого DATA состояния без копирования body bytes.
fn take_data(session: &mut Session) -> (Vec<u8>, String, Vec<String>) {
    let SmtpState::Data {
        bytes,
        reverse_path,
        recipients,
        ..
    } = &mut session.smtp
    else {
        return (Vec::new(), String::new(), Vec::new());
    };
    (
        std::mem::take(bytes),
        std::mem::take(reverse_path),
        std::mem::take(recipients),
    )
}

/// Добавляет unstuffed octets только пока DATA соблюдает лимиты профиля.
fn append_data_line(
    bytes: &mut Vec<u8>,
    oversized: &mut bool,
    mode: smtp::BodyMode,
    line: &[u8],
) -> io::Result<()> {
    let content = match line.strip_prefix(b"..") {
        Some(unstuffed) => unstuffed,
        None => line,
    };
    match data_line_accepts(mode, content) {
        Err(()) => *oversized = true,
        Ok(()) => match bytes.len().saturating_add(content.len()) > MAX_MESSAGE {
            true => *oversized = true,
            false => bytes.extend_from_slice(content),
        },
    }
    Ok(())
}

/// Соблюдает минимум RFC 5321 и 7BIT/8BITMIME octet mode.
fn data_line_accepts(mode: smtp::BodyMode, content: &[u8]) -> Result<(), ()> {
    let line_limit_exceeded = content.len().saturating_add(2) > 1000;
    let non_ascii_without_extension = match mode {
        smtp::BodyMode::SevenBit => content.iter().any(|octet| !octet.is_ascii()),
        smtp::BodyMode::EightBitMime => false,
    };
    match (line_limit_exceeded, non_ascii_without_extension) {
        (true, _) | (_, true) => Err(()),
        (false, false) => Ok(()),
    }
}

/// Выдаёт следующий ненулевой request id без дублирования арифметики.
fn next_request(request_id: &mut u64) -> u64 {
    let current = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    current
}
