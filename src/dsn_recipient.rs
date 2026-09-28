//! Типизированное представление адреса конечного получателя DSN.

/// Выбирает DSN address type по кодировке конечного адреса mailbox.
pub(super) fn final_recipient_type(recipient: &str) -> &'static str {
    match recipient.is_ascii() {
        true => "rfc822",
        false => "utf-8",
    }
}
