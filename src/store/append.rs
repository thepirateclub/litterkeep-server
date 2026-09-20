//! Публикация immutable blob, mailbox links и исходящих jobs одной транзакцией.
use super::{
    AppendMetadata, AppendStage, Store, WriteState, blobs, commit_log, durability, journal, model,
    recent, sieve_diagnostics, validated_unique_mailboxes,
};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

impl Store {
    pub fn append(&mut self, mailbox: &str, raw: &[u8]) -> io::Result<u64> {
        self.append_many(&[mailbox.to_owned()], raw)
    }
    pub fn append_with_metadata(
        &mut self,
        mailbox: &str,
        raw: &[u8],
        metadata: AppendMetadata,
    ) -> io::Result<u64> {
        self.append_many_with_metadata(&[mailbox.to_owned()], raw, metadata, &[], &[])
    }
    pub fn append_many(&mut self, mailboxes: &[String], raw: &[u8]) -> io::Result<u64> {
        self.append_many_with_metadata(mailboxes, raw, AppendMetadata::default(), &[], &[])
    }
    #[cfg(test)]
    pub(crate) fn append_many_with_outbound(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        outbound: &[(String, String)],
    ) -> io::Result<u64> {
        self.append_many_with_metadata(mailboxes, raw, AppendMetadata::default(), outbound, &[])
    }
    pub(super) fn append_many_with_sieve(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        outbound: &[(String, String)],
        diagnostics: &[sieve_diagnostics::PendingDiagnostic],
    ) -> io::Result<u64> {
        self.append_many_with_metadata(
            mailboxes,
            raw,
            AppendMetadata::default(),
            outbound,
            diagnostics,
        )
    }
    fn append_many_with_metadata(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        metadata: AppendMetadata,
        outbound: &[(String, String)],
        diagnostics: &[sieve_diagnostics::PendingDiagnostic],
    ) -> io::Result<u64> {
        self.ensure_writable()?;
        let mailboxes = validated_delivery_mailboxes(mailboxes, outbound)?;
        self.blob_quota.ensure_available(raw.len())?;
        let mut transaction_id = self.next_transaction_id;
        let (name, _) = blobs::next_available(&self.root, &mut transaction_id)?;
        let links = self.prepare_links(&mailboxes, transaction_id, &name, metadata)?;
        blobs::publish(&self.root, &name, raw)?;
        self.check_and_poison(AppendStage::BlobPublished)?;
        self.blob_quota.record(raw.len())?;
        for (sender, recipient) in outbound {
            self.queue
                .enqueue_owned(transaction_id, sender, recipient)?;
        }
        self.check_and_poison(AppendStage::QueueWritten)?;
        if let Err(error) = sieve_diagnostics::append(&self.root, transaction_id, diagnostics) {
            self.write_state = WriteState::Poisoned;
            return Err(error);
        }
        self.check_and_poison(AppendStage::DiagnosticsWritten)?;
        match self.commit_links(transaction_id, &links) {
            Ok(()) => {
                self.check_and_poison(AppendStage::BeforeMemoryPublish)?;
                self.publish_links(transaction_id, links)
            }
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
    }
    fn check_and_poison(&mut self, stage: AppendStage) -> io::Result<()> {
        match self.check_failpoint(stage) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
    }
    pub(super) fn index(&self, mailbox: &str) -> PathBuf {
        self.root
            .join("mailboxes")
            .join(format!("{mailbox}.journal"))
    }
    pub(super) fn ensure_writable(&self) -> io::Result<()> {
        match self.write_state {
            WriteState::Writable => Ok(()),
            WriteState::Poisoned => Err(io::Error::other("store writer is poisoned")),
        }
    }
    fn prepare_links(
        &self,
        mailboxes: &[String],
        transaction_id: u64,
        name: &str,
        metadata: AppendMetadata,
    ) -> io::Result<Vec<(String, model::Record)>> {
        mailboxes
            .iter()
            .map(|mailbox| {
                let uid = self.next_uids.get(mailbox).copied().unwrap_or(1);
                uid.checked_add(1)
                    .ok_or_else(|| io::Error::other("uid exhausted"))?;
                Ok((
                    mailbox.clone(),
                    model::Record {
                        uid,
                        transaction_id,
                        blob_transaction_id: transaction_id,
                        name: name.to_owned(),
                        flags: metadata.flags,
                        internal_date: metadata.internal_date,
                    },
                ))
            })
            .collect()
    }
    pub(super) fn commit_links(
        &self,
        transaction_id: u64,
        links: &[(String, model::Record)],
    ) -> io::Result<()> {
        let mut written = 0;
        for (mailbox, record) in links {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.index(mailbox))?;
            file.write_all(&journal::frame(record))?;
            file.sync_all()?;
            written += 1;
            if written == 1 {
                self.check_failpoint(AppendStage::FirstMailboxFrame)?;
            }
        }
        durability::sync_directory(&self.root.join("mailboxes"))?;
        self.check_failpoint(AppendStage::MailboxDirectorySynced)?;
        recent::add(&self.root, transaction_id, links)?;
        self.check_failpoint(AppendStage::RecentWritten)?;
        let mut commits = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("journal/transactions.log"))?;
        commits.write_all(&commit_log::frame(transaction_id))?;
        commits.sync_all()?;
        self.check_failpoint(AppendStage::CommitFrameSynced)?;
        durability::sync_directory(&self.root.join("journal"))?;
        self.check_failpoint(AppendStage::CommitDirectorySynced)
    }
    fn publish_links(
        &mut self,
        transaction_id: u64,
        links: Vec<(String, model::Record)>,
    ) -> io::Result<u64> {
        let first_uid = match links.first() {
            Some((_, record)) => record.uid,
            None => transaction_id,
        };
        for (mailbox, record) in links {
            let next_uid = record
                .uid
                .checked_add(1)
                .ok_or_else(|| io::Error::other("uid exhausted"))?;
            self.next_uids.insert(mailbox.clone(), next_uid);
            self.recent
                .entry(mailbox.clone())
                .or_default()
                .insert(record.uid);
            self.entries.entry(mailbox).or_default().push(record);
        }
        self.next_transaction_id = transaction_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?;
        self.committed_transactions.insert(transaction_id);
        Ok(first_uid)
    }

    /// Возвращает только jobs, пережившие global Store commit и recovery.
    pub fn pending_outbound(&self) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        self.queue.pending_owned(&self.committed_transactions)
    }

    /// Возвращает только committed jobs, чей durable retry deadline уже наступил.
    ///
    /// Transport не интерпретирует queue journal самостоятельно и не получает
    /// доступ к Store path: решение о видимости job остаётся у владельца Store.
    pub fn due_outbound(&self, now_epoch: u64) -> io::Result<Vec<crate::queue::OwnedDelivery>> {
        self.queue
            .due_owned(&self.committed_transactions, now_epoch)
    }

    /// Выдаёт не более `limit` due jobs, сохраняя bounded handoff к scheduler-у.
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
        match self
            .committed_transactions
            .contains(&delivery.transaction_id)
        {
            true => fs::read(
                self.root
                    .join("blobs")
                    .join(model::blob_name(delivery.transaction_id)),
            ),
            false => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "outbound job is not committed",
            )),
        }
    }

    /// Сохраняет terminal либо retry state только через владельца Store.
    pub fn transition_outbound(
        &mut self,
        delivery: &crate::queue::OwnedDelivery,
        state: crate::queue::State,
    ) -> io::Result<()> {
        self.ensure_writable()?;
        match self
            .committed_transactions
            .contains(&delivery.transaction_id)
        {
            true => self.queue.transition_owned(delivery, state),
            false => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "outbound job is not committed",
            )),
        }
    }
}

/// Разрешает redirect-only transaction, но не пустой обычный APPEND.
fn validated_delivery_mailboxes(
    mailboxes: &[String],
    outbound: &[(String, String)],
) -> io::Result<Vec<String>> {
    match (mailboxes.is_empty(), outbound.is_empty()) {
        (false, _) => validated_unique_mailboxes(mailboxes),
        (true, false) => Ok(Vec::new()),
        (true, true) => Err(io::Error::new(io::ErrorKind::InvalidInput, "no recipients")),
    }
}
