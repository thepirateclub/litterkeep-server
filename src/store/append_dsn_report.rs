//! Идемпотентная публикация failure DSN через общую Store-транзакцию.

use super::{AppendExtensions, AppendMetadata, Store};
use std::io;

impl Store {
    /// Публикует один failure DSN для queue job и возвращает его transaction ID.
    pub(crate) fn append_dsn_report_once(
        &mut self,
        mailbox: &str,
        raw: &[u8],
        key: &crate::queue::DsnReportKey,
    ) -> io::Result<Option<u64>> {
        if let Some(transaction_id) = self.dsn_report_transactions.get(key) {
            return Ok(Some(*transaction_id));
        }
        self.append_many_with_metadata(
            &[mailbox.to_owned()],
            raw,
            AppendMetadata::default(),
            &[],
            AppendExtensions {
                dsn_report_key: Some(key),
                ..AppendExtensions::default()
            },
        )
        .map(Some)
    }
}
