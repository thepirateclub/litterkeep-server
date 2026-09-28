//! Доступ владельца Store к pending local-success уведомлениям.

use super::{Store, model};
use std::{fs, io};

impl Store {
    /// Возвращает bounded cursor-страницу и оборачивается в начало при EOF.
    pub(crate) fn pending_dsn_success_notices_after(
        &self,
        cursor: Option<crate::queue::DsnSuccessKey>,
        limit: usize,
    ) -> Vec<crate::queue::DsnSuccessKey> {
        let mut notices = Vec::with_capacity(limit);
        let mut later = match cursor {
            Some(cursor) => self
                .dsn_success_notices
                .range::<crate::queue::DsnSuccessKey, _>((
                    std::ops::Bound::Excluded(cursor),
                    std::ops::Bound::Unbounded,
                )),
            None => self
                .dsn_success_notices
                .range::<crate::queue::DsnSuccessKey, _>((
                    std::ops::Bound::<crate::queue::DsnSuccessKey>::Unbounded,
                    std::ops::Bound::<crate::queue::DsnSuccessKey>::Unbounded,
                )),
        };
        collect_pending(&mut notices, &mut later, limit);
        if let Some(cursor) = cursor
            && notices.len() < limit
        {
            let remaining = limit.saturating_sub(notices.len());
            let mut earlier = self
                .dsn_success_notices
                .range::<crate::queue::DsnSuccessKey, _>((
                    std::ops::Bound::Unbounded,
                    std::ops::Bound::Excluded(cursor),
                ));
            collect_pending(&mut notices, &mut earlier, remaining);
        }
        notices
    }

    /// Возвращает RCPT success intents после исходного global commit.
    pub fn pending_dsn_success_notices(&self) -> Vec<crate::queue::DsnSuccessKey> {
        self.dsn_success_notices.iter().copied().collect::<Vec<_>>()
    }

    /// Восстанавливает исходные octets и recipient metadata одного pending intent.
    pub(crate) fn dsn_success_context(
        &self,
        key: crate::queue::DsnSuccessKey,
    ) -> io::Result<crate::queue::DsnSuccessContext> {
        if !self.dsn_success_notices.contains(&key) {
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

/// Добавляет только необработанные ключи, сохраняя BTree-порядок и лимит.
fn collect_pending<'a>(
    output: &mut Vec<crate::queue::DsnSuccessKey>,
    notices: &mut impl Iterator<Item = &'a crate::queue::DsnSuccessKey>,
    limit: usize,
) {
    for notice in notices {
        if output.len() >= limit {
            break;
        }
        output.push(*notice);
    }
}
