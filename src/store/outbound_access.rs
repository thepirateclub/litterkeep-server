//! Доступ transport scheduler-а к committed очереди Store.

use super::{Store, model};
use std::fs;
use std::io;

impl Store {
    /// Возвращает только jobs, пережившие global Store commit и recovery.
    pub fn pending_outbound(&self) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        let jobs = self.queue.pending_owned(&self.committed_transactions)?;
        self.attach_dsn_metadata(jobs)
    }

    /// Возвращает committed jobs с наступившим retry deadline.
    pub fn due_outbound(&self, now_epoch: u64) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        let jobs = self
            .queue
            .due_owned(&self.committed_transactions, now_epoch)?;
        self.attach_dsn_metadata(jobs)
    }

    /// Ограничивает число jobs, передаваемых transport scheduler-у.
    pub fn due_outbound_limited(
        &self,
        now_epoch: u64,
        limit: usize,
    ) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        let mut due = self.due_outbound(now_epoch)?;
        due.truncate(limit);
        Ok(due)
    }

    /// Связывает committed queue jobs с sidecar DSN через transaction и RCPT ID.
    fn attach_dsn_metadata(
        &self,
        jobs: Vec<crate::queue::OwnedDelivery>,
    ) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        jobs.into_iter()
            .map(|mut delivery| {
                let Some(envelope) = self.smtp_dsn_envelopes.get(&delivery.transaction_id) else {
                    return Ok(delivery);
                };
                let origin_recipients = delivery
                    .original_recipient_indices
                    .iter()
                    .map(|index| {
                        envelope
                            .recipients
                            .get(*index)
                            .map(|value| Some(value.dsn.clone()))
                            .ok_or_else(invalid_dsn_index)
                    })
                    .collect::<io::Result<Vec<_>>>()?;
                let recipient = origin_recipients.first().cloned().flatten();
                delivery.dsn_metadata = Some(Box::new(crate::queue::DsnDeliveryMetadata {
                    mail: envelope.mail.clone(),
                    recipient,
                    origin_recipients,
                }));
                Ok(delivery)
            })
            .collect()
    }

    /// Читает payload только job, опубликованный общим transaction commit.
    pub fn outbound_blob(&self, delivery: &crate::queue::OwnedDelivery) -> io::Result<Vec<u8>> {
        if !self
            .committed_transactions
            .contains(&delivery.transaction_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "outbound job is not committed",
            ));
        }
        fs::read(
            self.root
                .join("blobs")
                .join(model::blob_name(delivery.transaction_id)),
        )
    }

    /// Сохраняет terminal либо retry state только через владельца Store.
    pub fn transition_outbound(
        &mut self,
        delivery: &crate::queue::OwnedDelivery,
        state: crate::queue::State,
    ) -> io::Result<()> {
        self.ensure_writable()?;
        if !self
            .committed_transactions
            .contains(&delivery.transaction_id)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "outbound job is not committed",
            ));
        }
        self.queue.transition_owned(delivery, state)
    }
}

/// Сообщает о повреждённом индексе, не подменяя политику другим RCPT.
fn invalid_dsn_index() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "outbound recipient index is outside DSN envelope",
    )
}
