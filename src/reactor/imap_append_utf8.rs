//! Проверка RFC 9755 APPEND до передачи сообщения владельцу хранилища.

/// Определяет, содержит ли область заголовков 8-bit octet.
///
/// RFC 9755 требует отклонять такое APPEND с `NO`, если клиент не включил
/// UTF8=ACCEPT. Поиск ограничен первым разделителем CRLF CRLF, поэтому
/// бинарное или UTF-8 тело сообщения не влияет на решение.
pub(super) fn has_eight_bit_header(message: &[u8]) -> bool {
    if message.starts_with(b"\r\n") {
        return false;
    }
    let Some(separator) = message.windows(4).position(|window| window == b"\r\n\r\n") else {
        return message.iter().any(|octet| !octet.is_ascii());
    };
    message
        .get(..separator)
        .is_some_and(|header| header.iter().any(|octet| !octet.is_ascii()))
}

#[cfg(test)]
#[path = "imap_append_utf8_tests.rs"]
mod tests;
