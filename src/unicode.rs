//! Общие Unicode-преобразования, применяемые протокольными и storage слоями.

/// NFC-реализация привязана к версиям Unicode, согласованным с Rust toolchain.
#[path = "imap/unicode_17_nfc.rs"]
pub(crate) mod nfc;
