//! Доступ transport scheduler-а к committed очереди Store.

use super::{Store, model};
use std::fs;
use std::io;

impl Store {
    /// Возвращает только jobs, пережившие global Store commit и recovery.
    pub fn pending_outbound(&self) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        self.queue.pending_owned(&self.committed_transactions)
    }

    /// Возвращает committed jobs с наступившим retry deadline.
    pub fn due_outbound(&self, now_epoch: u64) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        self.queue
            .due_owned(&self.committed_transactions, now_epoch)
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
