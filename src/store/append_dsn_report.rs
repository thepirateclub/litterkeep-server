//! Идемпотентная публикация failure DSN через общую Store-транзакцию.

use super::{AppendExtensions, AppendMetadata, Store};
use std::io;

impl Store {
    /// Публикует один failure DSN для queue job и возвращает его transaction ID.
    pub(crate) fn append_dsn_report_once(
        &mut self,
        target: crate::dsn_target::DsnTarget,
        raw: &[u8],
        key: &crate::queue::DsnReportKey,
    ) -> io::Result<Option<u64>> {
        if let Some(transaction_id) = self.dsn_report_transactions.get(key) {
            return Ok(Some(*transaction_id));
        }
        let (mailboxes, outbound) = match target {
            crate::dsn_target::DsnTarget::LocalMailbox(mailbox) => (vec![mailbox], Vec::new()),
            crate::dsn_target::DsnTarget::OutboundAddress(recipient) => {
                (Vec::new(), vec![(String::new(), recipient)])
            }
        };
        self.append_many_with_metadata(
            &mailboxes,
            raw,
            AppendMetadata::default(),
            &outbound,
            AppendExtensions {
                dsn_report_key: Some(key),
                ..AppendExtensions::default()
            },
        )
        .map(Some)
    }
}
