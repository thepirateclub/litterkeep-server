//! Порядок durable записей одной append transaction.

use super::super::dsn_success_journal;
use super::{
    AppendExtensions, AppendMetadata, AppendStage, Store, WriteState, blobs, dsn_report_journal,
    outbound, sieve_diagnostics, smtp_dsn_journal, validation,
};
use std::io;

/// Записывает blob, queue и metadata до commit, затем публикует memory view.
pub(super) fn append(
    store: &mut Store,
    mailboxes: &[String],
    raw: &[u8],
    metadata: AppendMetadata,
    outbound_jobs: &[(String, String)],
    extensions: AppendExtensions<'_>,
) -> io::Result<u64> {
    store.ensure_writable()?;
    outbound::validate_count(outbound_jobs, extensions.outbound_origins)?;
    let mailboxes = validation::validated_delivery_mailboxes(mailboxes, outbound_jobs)?;
    store.blob_quota.ensure_available(raw.len())?;
    let mut transaction_id = store.next_transaction_id;
    let (name, _) = blobs::next_available(&store.root, &mut transaction_id)?;
    let links = store.prepare_links(&mailboxes, transaction_id, &name, metadata)?;
    blobs::publish(&store.root, &name, raw)?;
    store.check_and_poison(AppendStage::BlobPublished)?;
    store.blob_quota.record(raw.len())?;
    outbound::enqueue(
        store,
        transaction_id,
        outbound_jobs,
        extensions.outbound_origins,
    )?;
    store.check_and_poison(AppendStage::QueueWritten)?;
    append_diagnostics(store, transaction_id, extensions.diagnostics)?;
    append_smtp_envelope(store, transaction_id, extensions.smtp_dsn)?;
    append_report_key(store, transaction_id, extensions.dsn_report_key)?;
    append_success_intents(
        store,
        transaction_id,
        extensions.smtp_dsn,
        extensions.dsn_success_recipient_indices,
    )?;
    append_vacation_claim(store, transaction_id, extensions.vacation_claim)?;
    match store.commit_links(transaction_id, &links) {
        Ok(()) => {
            store.check_and_poison(AppendStage::BeforeMemoryPublish)?;
            let uid = store.publish_links(
                transaction_id,
                links,
                extensions.vacation_claim.cloned(),
                extensions.smtp_dsn.cloned(),
                None,
            )?;
            store.publish_dsn_report_key_before_publish(extensions.dsn_report_key, transaction_id);
            store
                .dsn_success_notices
                .extend(
                    extensions
                        .dsn_success_recipient_indices
                        .iter()
                        .map(|index| crate::queue::DsnSuccessKey {
                            transaction_id,
                            recipient_index: *index,
                        }),
                );
            Ok(uid)
        }
        Err(error) => {
            store.write_state = WriteState::Poisoned;
            Err(error)
        }
    }
}

fn append_diagnostics(
    store: &mut Store,
    transaction_id: u64,
    diagnostics: &[super::sieve_diagnostics::PendingDiagnostic],
) -> io::Result<()> {
    if let Err(error) = sieve_diagnostics::append(&store.root, transaction_id, diagnostics) {
        store.write_state = WriteState::Poisoned;
        return Err(error);
    }
    store.check_and_poison(AppendStage::DiagnosticsWritten)
}

fn append_smtp_envelope(
    store: &mut Store,
    transaction_id: u64,
    envelope: Option<&crate::smtp::DsnEnvelope>,
) -> io::Result<()> {
    if let Some(envelope) = envelope
        && let Err(error) = smtp_dsn_journal::append(&store.root, transaction_id, envelope)
    {
        store.write_state = WriteState::Poisoned;
        return Err(error);
    }
    store.check_and_poison(AppendStage::EnvelopeWritten)
}

fn append_report_key(
    store: &mut Store,
    transaction_id: u64,
    key: Option<&crate::queue::DsnReportKey>,
) -> io::Result<()> {
    if let Some(key) = key
        && let Err(error) = dsn_report_journal::append(&store.root, transaction_id, key)
    {
        store.write_state = WriteState::Poisoned;
        return Err(error);
    }
    store.check_and_poison(AppendStage::DsnReportWritten)
}

fn append_success_intents(
    store: &mut Store,
    transaction_id: u64,
    envelope: Option<&crate::smtp::DsnEnvelope>,
    recipient_indices: &[usize],
) -> io::Result<()> {
    if !recipient_indices.is_empty() {
        let valid_envelope = envelope.is_some_and(|value| {
            recipient_indices.iter().all(|index| {
                value.recipients.get(*index).is_some_and(|recipient| {
                    crate::dsn::notification_allowed(
                        Some(&crate::queue::DsnDeliveryMetadata {
                            mail: value.mail.clone(),
                            recipient: Some(recipient.dsn.clone()),
                        }),
                        crate::smtp::NotifyCondition::Success,
                    )
                })
            })
        });
        if !valid_envelope {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "success notice lacks matching NOTIFY=SUCCESS recipient",
            ));
        }
    }
    if let Err(error) =
        dsn_success_journal::append(store.root.as_path(), transaction_id, recipient_indices)
    {
        store.write_state = WriteState::Poisoned;
        return Err(error);
    }
    store.check_and_poison(AppendStage::DsnSuccessWritten)
}

fn append_vacation_claim(
    store: &mut Store,
    transaction_id: u64,
    claim: Option<&super::super::vacation_suppression::PendingClaim>,
) -> io::Result<()> {
    if let Err(error) = super::super::vacation_suppression::append(
        &store.root,
        transaction_id,
        claim,
        &store.committed_transactions,
    ) {
        store.write_state = WriteState::Poisoned;
        return Err(error);
    }
    store.check_and_poison(AppendStage::VacationWritten)
}
