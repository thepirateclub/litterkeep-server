//! SMTP DATA octet handling and durable local-delivery handoff.
use super::session::DataSection;
use super::*;
use crate::local_route::local_mailboxes;

#[path = "smtp_data_finish.rs"]
mod finish;
#[path = "smtp_data_octets.rs"]
mod octets;

/// Завершённое MIME-сообщение с SMTP envelope для bounded Store request.
pub(super) struct Delivery {
    pub(super) bytes: Vec<u8>,
    pub(super) reverse_path: String,
    pub(super) recipients: Vec<String>,
    pub(super) dsn: smtp::DsnEnvelope,
}

/// Причина отказа DATA после полного приёма dot-terminated транзакции.
pub(super) enum DataFailure {
    MessageTooLarge,
    InvalidOctets,
}

/// Отбрасывает заведомо слишком длинный незавершённый DATA-фрагмент.
///
/// RFC 5321 ограничивает строку после снятия transparency до 1000 октетов
/// вместе с CRLF. До появления CRLF допустимый wire-фрагмент не длиннее этого
/// предела, включая возможный transparency dot.
pub(super) fn discard_overlong_incomplete_line(input: &mut Vec<u8>, oversized: &mut bool) {
    let incomplete_start = input
        .windows(2)
        .rposition(|pair| pair == b"\r\n")
        .map_or(0, |end| end + 2);
    let incomplete_size = input.len().saturating_sub(incomplete_start);
    if incomplete_size > smtp::MAX_DATA_LINE_SIZE {
        *oversized = true;
        input.truncate(incomplete_start);
    }
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
            SmtpState::Command(_)
            | SmtpState::BdatDiscard { .. }
            | SmtpState::BdatReady(_)
            | SmtpState::BdatFailed
            | SmtpState::BdatReceiving(_) => false,
        };
        let invalid_octets = match &session.smtp {
            SmtpState::Data { invalid_octets, .. } => *invalid_octets,
            SmtpState::Command(_)
            | SmtpState::BdatDiscard { .. }
            | SmtpState::BdatReady(_)
            | SmtpState::BdatFailed
            | SmtpState::BdatReceiving(_) => false,
        };
        let failure = match (oversized, invalid_octets) {
            (true, _) => Some(DataFailure::MessageTooLarge),
            (false, true) => Some(DataFailure::InvalidOctets),
            (false, false) => None,
        };
        return finish::finish_data(
            session, slot, generation, worker, owners, request_id, failure,
        );
    }
    let SmtpState::Data {
        bytes,
        section,
        oversized,
        invalid_octets,
        body_mode,
        smtp_utf8,
        ..
    } = &mut session.smtp
    else {
        return Ok(());
    };
    match (*oversized, *invalid_octets) {
        (true, _) | (_, true) => Ok(()),
        (false, false) => append_data_line(
            bytes,
            section,
            oversized,
            invalid_octets,
            *body_mode,
            *smtp_utf8,
            &line,
        ),
    }
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
        Request::AppendMany(Box::new(crate::storage_worker::SmtpDeliveryRequest {
            request_id: id,
            mailboxes,
            envelope_recipients,
            envelope_sender: delivery.reverse_path,
            dsn: delivery.dsn,
            bytes: delivery.bytes,
        })),
        pending,
        slot,
        generation,
        worker,
        owners,
    );
    Ok(())
}

/// Подготавливает BDAT сообщение для того же durable AppendMany владельца.
pub(super) fn submit_bdat(
    session: &mut Session,
    mut delivery: Delivery,
    slot: usize,
    generation: u64,
    worker: &mut WakingStorageWorker,
    owners: &mut HashMap<u64, (usize, u64, Pending)>,
    request_id: &mut u64,
) -> io::Result<()> {
    delivery.bytes = crate::smtp_trace::prepend(
        &delivery.bytes,
        &delivery.reverse_path,
        std::time::SystemTime::now(),
    )?;
    submit_delivery(
        session, delivery, slot, generation, worker, owners, request_id,
    )
}

/// Переносит поля завершённого DATA состояния без копирования body bytes.
pub(super) fn take_data(
    session: &mut Session,
) -> (Vec<u8>, String, Vec<String>, smtp::DsnEnvelope) {
    let SmtpState::Data {
        bytes,
        reverse_path,
        recipients,
        dsn,
        ..
    } = &mut session.smtp
    else {
        return (
            Vec::new(),
            String::new(),
            Vec::new(),
            smtp::DsnEnvelope {
                reverse_path: String::new(),
                mail: Default::default(),
                recipients: Vec::new(),
            },
        );
    };
    (
        std::mem::take(bytes),
        std::mem::take(reverse_path),
        std::mem::take(recipients),
        std::mem::take(dsn),
    )
}

/// Добавляет unstuffed octets только пока DATA соблюдает лимиты профиля.
fn append_data_line(
    bytes: &mut Vec<u8>,
    section: &mut DataSection,
    oversized: &mut bool,
    invalid_octets: &mut bool,
    mode: smtp::BodyMode,
    smtp_utf8: bool,
    line: &[u8],
) -> io::Result<()> {
    let content = match line.strip_prefix(b".") {
        Some(unstuffed) => unstuffed,
        None => line,
    };
    let headers = matches!(*section, DataSection::Headers);
    match octets::validate_line_octets(content) {
        Ok(()) => {}
        Err(_) => {
            *invalid_octets = true;
            return Ok(());
        }
    }
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
pub(super) fn advance_section(section: &mut DataSection, content: &[u8]) {
    if let (DataSection::Headers, b"\r\n") = (*section, content) {
        *section = DataSection::Body;
    }
}

/// Соблюдает минимум RFC 5321 и 7BIT/8BITMIME octet mode.
pub(super) fn data_line_accepts(
    mode: smtp::BodyMode,
    smtp_utf8: bool,
    headers: bool,
    content: &[u8],
) -> Result<(), ()> {
    let line_limit_exceeded = content.len() > smtp::MAX_DATA_LINE_SIZE;
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
