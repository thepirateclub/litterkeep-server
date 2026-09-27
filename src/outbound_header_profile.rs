//! Определение профиля UTF-8 для RFC 6532 заголовков сообщения.

/// Выявляет UTF-8 octets в заголовках, требующие SMTPUTF8 negotiation.
///
/// Разделитель MIME-заголовков ищется как CRLF CRLF, чтобы содержимое body
/// не влияло на выбор SMTP extension. Некорректный или неполный header block
/// не считается основанием включать SMTPUTF8.
pub(crate) fn has_utf8_headers(message: &[u8]) -> bool {
    match message.windows(4).position(|window| window == b"\r\n\r\n") {
        Some(end) => message
            .get(..end)
            .is_some_and(|headers| headers.iter().any(|byte| !byte.is_ascii())),
        None => false,
    }
}
