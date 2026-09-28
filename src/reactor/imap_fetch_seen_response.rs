//! Сборка FETCH-ответа после успешного durable перехода Seen.

use super::*;

/// Записывает BODY FETCH с FLAGS после durable Seen update.
pub(super) fn write(
    session: &mut Session,
    number: usize,
    message: &crate::store::Message,
    attributes: &[imap::FetchAttribute],
    addressing: imap::Addressing,
) -> io::Result<Result<(), crate::imap_fetch::FetchError>> {
    let response = match imap_fetch_response::prepare_internal(
        number,
        message,
        attributes,
        addressing,
        &session.selected_recent,
        session.imap_string_mode,
        true,
    ) {
        Ok(response) => response,
        Err(reason) => return Ok(Err(reason)),
    };
    session.push(&response)?;
    Ok(Ok(()))
}

/// Подготавливает ответ для проверки до записи Seen в журнал.
pub(super) fn prepare(
    number: usize,
    message: &crate::store::Message,
    attributes: &[imap::FetchAttribute],
    addressing: imap::Addressing,
    selected_recent: &std::collections::BTreeSet<u64>,
) -> Result<Vec<u8>, crate::imap_fetch::FetchError> {
    imap_fetch_response::prepare_internal(
        number,
        message,
        attributes,
        addressing,
        selected_recent,
        imap::tokenizer::StringMode::Legacy,
        true,
    )
}
