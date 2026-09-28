//! Общие Unicode-преобразования, применяемые протокольными и storage слоями.

/// NFC-реализация привязана к версиям Unicode, согласованным с Rust toolchain.
#[path = "imap/unicode_17_nfc.rs"]
pub(crate) mod nfc;

/// Проверяет назначенность скаляра по закреплённым таблицам Unicode 17.0.0.
pub(crate) fn is_assigned(character: char) -> bool {
    crate::imap::unicode_17::is_assigned(character)
}
