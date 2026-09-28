//! Доступ владельца Store к pending local-success уведомлениям.

use super::{Store, model};
use std::{fs, io};

impl Store {
    /// Возвращает RCPT success intents после исходного global commit.
    pub fn pending_dsn_success_notices(&self) -> Vec<crate::queue::DsnSuccessKey> {
        let mut notices = self
            .dsn_success_notices
            .iter()
            .filter(|notice| {
                let key = crate::queue::DsnSuccessReportKey {
                    source_transaction_id: notice.transaction_id,
                    recipient_index: notice.recipient_index,
                };
                !self.dsn_success_report_transactions.contains_key(&key)
            })
            .copied()
            .collect::<Vec<_>>();
        notices.sort_by_key(|notice| (notice.transaction_id, notice.recipient_index));
        notices
    }

    /// Восстанавливает исходные octets и recipient metadata одного pending intent.
    pub(crate) fn dsn_success_context(
        &self,
        key: crate::queue::DsnSuccessKey,
    ) -> io::Result<crate::queue::DsnSuccessContext> {
        if !self.pending_dsn_success_notices().contains(&key) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "success notice is not pending",
            ));
        }
        let envelope = self
            .smtp_dsn_envelopes
            .get(&key.transaction_id)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "success notice has no SMTP envelope",
                )
            })?;
        let recipient = envelope
            .recipients
            .get(key.recipient_index)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "success recipient ordinal is invalid",
                )
            })?;
        Ok(crate::queue::DsnSuccessContext {
            reverse_path: envelope.reverse_path.clone(),
            recipient: recipient.address.clone(),
            original: fs::read(
                self.root
                    .join("blobs")
                    .join(model::blob_name(key.transaction_id)),
            )?,
            metadata: crate::queue::DsnDeliveryMetadata {
                mail: envelope.mail.clone(),
                recipient: Some(recipient.dsn.clone()),
            },
        })
    }
}
