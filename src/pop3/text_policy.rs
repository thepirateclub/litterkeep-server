//! Политика POP3-кодировок для LIST, TOP и RETR.

use super::maildrop::{Maildrop, MessageEncoding};
use super::session::{Reply, ResponseCode};

/// Проверяет, можно ли безопасно представить выбранные сообщения текущему peer.
pub(super) fn evaluate(
    drop: &Maildrop,
    number: Option<u32>,
    utf8_active: bool,
) -> Result<(), Reply> {
    match drop.encoding(number) {
        Ok(MessageEncoding::Ascii) => Ok(()),
        Ok(MessageEncoding::Utf8) if !utf8_active => Err(Reply::ErrorCode(
            ResponseCode::Utf8,
            "message contains UTF-8; issue UTF8 first",
        )),
        Ok(MessageEncoding::Opaque) if !utf8_active => {
            Err(Reply::Error("non-ASCII message requires UTF8 mode"))
        }
        Ok(MessageEncoding::Mixed) if !utf8_active => Err(Reply::ErrorCode(
            ResponseCode::Utf8,
            "message contains UTF-8; issue UTF8 first",
        )),
        Ok(MessageEncoding::Opaque | MessageEncoding::Mixed) => {
            Err(Reply::Error("message cannot be represented as valid UTF-8"))
        }
        Ok(MessageEncoding::Utf8) => Ok(()),
        Err(error) => Err(Reply::Error(error)),
    }
}
