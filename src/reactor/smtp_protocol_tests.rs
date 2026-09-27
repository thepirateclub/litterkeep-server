//! Проверки RFC 6409 sender authorization SMTP submission.

use super::sender_is_authorized;

/// Аутентифицированная личность задаёт return path для допустимого null sender.
#[test]
fn authenticated_identity_allows_null_reverse_path() {
    assert!(sender_is_authorized("", "alice"));
}

/// Ненулевой envelope sender по-прежнему должен принадлежать аккаунту.
#[test]
fn authenticated_identity_rejects_another_accounts_sender() {
    assert!(sender_is_authorized("alice@litterkeep.test", "alice"));
    assert!(!sender_is_authorized("bob@litterkeep.test", "alice"));
}
