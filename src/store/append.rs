//! Публикация immutable blob, mailbox links и исходящих jobs одной транзакцией.
use super::{
    AppendMetadata, AppendStage, Store, WriteState, blobs, commit_log, durability, journal, model,
    recent, sieve_diagnostics, smtp_dsn_journal,
};
#[path = "append_outbound.rs"]
mod outbound;
use std::fs::OpenOptions;
use std::io::{self, Write};

#[path = "append_dsn_report.rs"]
mod dsn_report;
#[path = "append_dsn_success_report.rs"]
mod dsn_success_report;
#[path = "append_transaction.rs"]
mod transaction;
use super::dsn_report_journal;
#[path = "append_validation.rs"]
mod validation;

/// Дополнительные durable данные одной append-транзакции.
#[derive(Default)]
pub(super) struct AppendExtensions<'a> {
    pub(super) diagnostics: &'a [sieve_diagnostics::PendingDiagnostic],
    pub(super) outbound_origins: &'a [Option<usize>],
    pub(super) vacation_claim: Option<&'a super::vacation_suppression::PendingClaim>,
    pub(super) smtp_dsn: Option<&'a crate::smtp::DsnEnvelope>,
    pub(super) dsn_report_key: Option<&'a crate::queue::DsnReportKey>,
    pub(super) dsn_success_recipient_indices: &'a [usize],
    pub(super) dsn_success_report_key: Option<crate::queue::DsnSuccessReportKey>,
}

/// Borrowed payload одного SMTP Sieve plan до атомарной Store публикации.
pub(super) struct SieveAppend<'a> {
    pub(super) mailboxes: &'a [String],
    pub(super) raw: &'a [u8],
    pub(super) outbound: &'a [(String, String)],
    pub(super) diagnostics: &'a [sieve_diagnostics::PendingDiagnostic],
    pub(super) smtp_dsn: Option<&'a crate::smtp::DsnEnvelope>,
    pub(super) outbound_origins: &'a [Option<usize>],
    pub(super) dsn_success_recipient_indices: &'a [usize],
}

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
        self.append_many_with_sieve(&[], raw, &outbound, &[], None, &[])
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
            AppendExtensions {
                vacation_claim: Some(claim),
                ..AppendExtensions::default()
            },
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
        self.append_many_with_metadata(
            &[mailbox.to_owned()],
            raw,
            metadata,
            &[],
            AppendExtensions::default(),
        )
    }
    pub fn append_many(&mut self, mailboxes: &[String], raw: &[u8]) -> io::Result<u64> {
        self.append_many_with_metadata(
            mailboxes,
            raw,
            AppendMetadata::default(),
            &[],
            AppendExtensions::default(),
        )
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
            AppendExtensions::default(),
        )
    }
    pub(super) fn append_many_with_sieve(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        outbound: &[(String, String)],
        diagnostics: &[sieve_diagnostics::PendingDiagnostic],
        smtp_dsn: Option<&crate::smtp::DsnEnvelope>,
        outbound_origins: &[Option<usize>],
    ) -> io::Result<u64> {
        self.append_many_with_sieve_and_success(SieveAppend {
            mailboxes,
            raw,
            outbound,
            diagnostics,
            smtp_dsn,
            outbound_origins,
            dsn_success_recipient_indices: &[],
        })
    }

    /// Сохраняет RCPT success intents в ту же commit-границу local delivery.
    pub(super) fn append_many_with_sieve_and_success(
        &mut self,
        request: SieveAppend<'_>,
    ) -> io::Result<u64> {
        self.append_many_with_metadata(
            request.mailboxes,
            request.raw,
            AppendMetadata::default(),
            request.outbound,
            AppendExtensions {
                diagnostics: request.diagnostics,
                outbound_origins: request.outbound_origins,
                smtp_dsn: request.smtp_dsn,
                dsn_success_recipient_indices: request.dsn_success_recipient_indices,
                ..AppendExtensions::default()
            },
        )
    }
    pub(super) fn append_many_with_metadata(
        &mut self,
        mailboxes: &[String],
        raw: &[u8],
        metadata: AppendMetadata,
        outbound: &[(String, String)],
        extensions: AppendExtensions<'_>,
    ) -> io::Result<u64> {
        transaction::append(self, mailboxes, raw, metadata, outbound, extensions)
    }
    pub(super) fn publish_dsn_report_key_before_publish(
        &mut self,
        key: Option<&crate::queue::DsnReportKey>,
        transaction_id: u64,
    ) {
        if let Some(key) = key {
            self.dsn_report_transactions
                .insert(key.clone(), transaction_id);
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
    pub(super) fn prepare_links(
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
    pub(in crate::store) fn publish_links(
        &mut self,
        transaction_id: u64,
        links: Vec<(String, model::Record)>,
        vacation_claim: Option<super::vacation_suppression::PendingClaim>,
        smtp_dsn: Option<crate::smtp::DsnEnvelope>,
        dsn_report_key: Option<crate::queue::DsnReportKey>,
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
        if let Some(envelope) = smtp_dsn {
            self.smtp_dsn_envelopes.insert(transaction_id, envelope);
        }
        let _ = dsn_report_key;
        if let Some(claim) = vacation_claim {
            self.vacation_history.insert(claim.key, claim.expires_at);
        }
        Ok(first_uid)
    }
}
