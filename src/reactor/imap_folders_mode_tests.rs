//! Проверки mode-aware кодирования mailbox имён в quoted IMAP responses.

use super::quote;
use crate::imap::tokenizer::StringMode;

/// UTF-8 mode сохраняет Unicode, а legacy использует modified UTF-7.
#[test]
fn quote_uses_the_session_string_mode() {
    assert_eq!(quote("旅行", StringMode::Utf8Accept), "\"旅行\"");
    assert_eq!(quote("旅行", StringMode::Legacy), "\"&ZcWITA-\"");
}

/// Оба режима одинаково экранируют синтаксис quoted-string.
#[test]
fn quote_escapes_protocol_delimiters_in_both_modes() {
    for mode in [StringMode::Legacy, StringMode::Utf8Accept] {
        assert_eq!(quote("a\\b\"c", mode), "\"a\\\\b\\\"c\"");
    }
}
