//! Публикация immutable blob, mailbox links и исходящих jobs одной транзакцией.
use super::{
    AppendMetadata, AppendStage, Store, WriteState, blobs, commit_log, durability, journal, model,
    recent, sieve_diagnostics, validated_unique_mailboxes,
};
use std::fs::OpenOptions;
use std::io::{self, Write};

impl Store {
    /// Фиксирует отдельный generated payload и outbound job без mailbox link.
    ///
    /// Эта граница позволяет автоответчику публиковать собственные RFC822
    /// bytes, не подменяя их blob-ом принятого входящего сообщения.
    pub fn enqueue_outbound_payload(
        &mut self,
        sender: &str,
        recipient: &str,
        raw: &[u8],
    ) -> io::Result<u64> {
        let outbound = [(sender.to_owned(), recipient.to_owned())];
        self.append_many_with_sieve(&[], raw, &outbound, &[])
    }

    pub(super) fn append_vacation_transaction(
        &mut self,
        raw: &[u8],
        outbound: &[(String, String)],
        claim: &super::vacation_suppression::PendingClaim,
    ) -> io::Result<u64> {
        self.append_many_with_metadata(
            &[],
            raw,
            AppendMetadata::default(),
            outbound,
            &[],
            Some(claim),
        )
    }

    pub fn append(&mut self, mailbox: &str, raw: &[u8]) -> io::Result<u64> {
        self.append_many(&[mailbox.to_owned()], raw)
    }
    pub fn append_with_metadata(
        &mut self,
        mailbox: &str,
        raw: &[u8],
        metadata: AppendMetadata,
    ) -> io::Result<u64> {
        self.append_many_with_metadata(&[mailbox.to_owned()], raw, metadata, &[], &[], None)
    }
    pub fn append_many(&mut self, mailboxes: &[String], raw: &[u8]) -> io::Result<u64> {
        self.append_many_with_metadata(mailboxes, raw, AppendMetadata::default(), &[], &[], None)
    }
    #[cfg(test)]
    pub(crate) fn append_many_with_outbound(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        outbound: &[(String, String)],
    ) -> io::Result<u64> {
        self.append_many_with_metadata(
            mailboxes,
            raw,
            AppendMetadata::default(),
            outbound,
            &[],
            None,
        )
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
            None,
        )
    }
    fn append_many_with_metadata(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        metadata: AppendMetadata,
        outbound: &[(String, String)],
        diagnostics: &[sieve_diagnostics::PendingDiagnostic],
        vacation_claim: Option<&super::vacation_suppression::PendingClaim>,
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
        if let Err(error) = super::vacation_suppression::append(
            &self.root,
            transaction_id,
            vacation_claim,
            &self.committed_transactions,
        ) {
            self.write_state = WriteState::Poisoned;
            return Err(error);
        }
        self.check_and_poison(AppendStage::VacationWritten)?;
        match self.commit_links(transaction_id, &links) {
            Ok(()) => {
                self.check_and_poison(AppendStage::BeforeMemoryPublish)?;
                self.publish_links(transaction_id, links, vacation_claim.cloned())
            }
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
    }
    pub(super) fn check_and_poison(&mut self, stage: AppendStage) -> io::Result<()> {
        match self.check_failpoint(stage) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                Err(error)
            }
        }
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
                        keywords: metadata.keywords.clone(),
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
            let index = super::file_key::prepare(
                &self.root.join("mailboxes"),
                mailbox,
                super::file_key::FileKind::Journal,
            )?;
            let mut file = OpenOptions::new().create(true).append(true).open(&index)?;
            file.write_all(&journal::frame(record))?;
            file.sync_all()?;
            written += 1;
            if written == 1 {
                self.check_failpoint(AppendStage::FirstMailboxFrame)?;
            }
            if let Some(parent) = index.parent() {
                durability::sync_directory(parent)?;
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
    pub(super) fn publish_links(
        &mut self,
        transaction_id: u64,
        links: Vec<(String, model::Record)>,
        vacation_claim: Option<super::vacation_suppression::PendingClaim>,
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
        if let Some(claim) = vacation_claim {
            self.vacation_history.insert(claim.key, claim.expires_at);
        }
        Ok(first_uid)
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
