//! SMTP DATA octet handling and durable local-delivery handoff.
use super::session::DataSection;
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
        section,
        oversized,
        body_mode,
        smtp_utf8,
        ..
    } = &mut session.smtp
    else {
        return Ok(());
    };
    match *oversized {
        true => Ok(()),
        false => append_data_line(bytes, section, oversized, *body_mode, *smtp_utf8, &line),
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
        session.smtp_utf8 = false;
        return session.push(b"552 5.3.4 message too large\r\n");
    }
    let (bytes, reverse_path, recipients) = take_data(session);
    session.smtp = SmtpState::Command(smtp::Transaction::default());
    session.smtp_body_mode = smtp::BodyMode::SevenBit;
    session.smtp_utf8 = false;
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
    section: &mut DataSection,
    oversized: &mut bool,
    mode: smtp::BodyMode,
    smtp_utf8: bool,
    line: &[u8],
) -> io::Result<()> {
    let content = match line.strip_prefix(b".") {
        Some(unstuffed) => unstuffed,
        None => line,
    };
    let headers = matches!(*section, DataSection::Headers);
    match data_line_accepts(mode, smtp_utf8, headers, content) {
        Err(()) => *oversized = true,
        Ok(()) => match bytes.len().saturating_add(content.len()) > MAX_MESSAGE {
            true => *oversized = true,
            false => {
                bytes.extend_from_slice(content);
                advance_section(section, content);
            }
        },
    }
    Ok(())
}

/// Переключает профиль в body при пустой DATA-строке CRLF.
fn advance_section(section: &mut DataSection, content: &[u8]) {
    if let (DataSection::Headers, b"\r\n") = (*section, content) {
        *section = DataSection::Body;
    }
}

/// Соблюдает минимум RFC 5321 и 7BIT/8BITMIME octet mode.
fn data_line_accepts(
    mode: smtp::BodyMode,
    smtp_utf8: bool,
    headers: bool,
    content: &[u8],
) -> Result<(), ()> {
    let line_limit_exceeded = content.len() > 1000;
    let non_ascii = content.iter().any(|octet| !octet.is_ascii());
    let non_ascii_without_extension = match (headers, non_ascii, smtp_utf8, mode) {
        (_, false, _, _) => false,
        (true, true, true, _) => !valid_utf8_header_line(content),
        (false, true, _, smtp::BodyMode::EightBitMime) => false,
        _ => true,
    };
    match (line_limit_exceeded, non_ascii_without_extension) {
        (true, _) | (_, true) => Err(()),
        (false, false) => Ok(()),
    }
}

/// RFC 6532 допускает UTF-8 в поле заголовка, но не в имени поля.
fn valid_utf8_header_line(content: &[u8]) -> bool {
    if std::str::from_utf8(content).is_err() {
        return false;
    }
    match content.first() {
        Some(b' ' | b'\t') => true,
        _ => match content.iter().position(|octet| *octet == b':') {
            Some(separator) => content
                .get(..separator)
                .is_some_and(|name| !name.is_empty() && name.iter().all(valid_field_name_octet)),
            None => false,
        },
    }
}

/// Проверяет ASCII ftext без materialized преобразования заголовочной строки.
fn valid_field_name_octet(octet: &u8) -> bool {
    matches!(*octet, b'!'..=b'9' | b';'..=b'~')
}

/// Выдаёт следующий ненулевой request id без дублирования арифметики.
fn next_request(request_id: &mut u64) -> u64 {
    let current = *request_id;
    *request_id = request_id.wrapping_add(1).max(1);
    current
}

#[cfg(test)]
#[path = "smtp_data_tests.rs"]
mod tests;
