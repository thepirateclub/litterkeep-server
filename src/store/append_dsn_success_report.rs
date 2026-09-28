//! Идемпотентное добавление committed positive DSN в mailbox отправителя.

use super::{AppendExtensions, AppendMetadata, Store};
use std::io;

impl Store {
    /// Возвращает committed report transaction для повторной worker-команды.
    pub(crate) fn dsn_success_report_transaction(
        &self,
        key: &crate::queue::DsnSuccessReportKey,
    ) -> Option<u64> {
        self.dsn_success_report_transactions.get(key).copied()
    }

    /// Добавляет delivered report ровно один раз для исходного RCPT intent.
    pub(crate) fn append_dsn_success_report_once(
        &mut self,
        target: crate::dsn_target::DsnSuccessTarget,
        raw: &[u8],
        key: crate::queue::DsnSuccessReportKey,
    ) -> io::Result<u64> {
        if let Some(transaction_id) = self.dsn_success_report_transactions.get(&key) {
            return Ok(*transaction_id);
        }
        let intent = crate::queue::DsnSuccessKey {
            transaction_id: key.source_transaction_id,
            recipient_index: key.recipient_index,
        };
        if !self.dsn_success_notices.contains(&intent)
            || !self
                .committed_transactions
                .contains(&key.source_transaction_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "success report does not match a committed delivery intent",
            ));
        }
        let (mailboxes, outbound) = match target {
            crate::dsn_target::DsnSuccessTarget::LocalMailbox(mailbox) => {
                (vec![mailbox], Vec::new())
            }
            crate::dsn_target::DsnSuccessTarget::OutboundAddress(recipient) => {
                (Vec::new(), vec![(String::new(), recipient)])
            }
        };
        self.append_many_with_metadata(
            &mailboxes,
            raw,
            AppendMetadata::default(),
            &outbound,
            AppendExtensions {
                dsn_success_report_key: Some(key),
                ..AppendExtensions::default()
            },
        )?;
        self.dsn_success_report_transactions
            .get(&key)
            .copied()
            .ok_or_else(|| io::Error::other("committed success report key was not published"))
    }
}
