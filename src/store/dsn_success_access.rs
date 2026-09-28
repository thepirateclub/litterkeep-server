//! Доступ владельца Store к pending local-success уведомлениям.

use super::Store;

impl Store {
    /// Возвращает RCPT success intents после исходного global commit.
    pub fn pending_dsn_success_notices(&self) -> Vec<crate::queue::DsnSuccessKey> {
        let mut notices = self.dsn_success_notices.iter().copied().collect::<Vec<_>>();
        notices.sort_by_key(|notice| (notice.transaction_id, notice.recipient_index));
        notices
    }
}
