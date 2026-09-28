//! Атомарная публикация нескольких сообщений одного mailbox одной транзакцией.

use super::{AppendMetadata, AppendStage, Store, WriteState, blobs, model};
use std::io;

impl Store {
    /// Фиксирует пакет так, чтобы recovery видел весь набор сообщений или ни одного.
    pub fn append_batch(
        &mut self,
        mailbox: &str,
        messages: &[(Vec<u8>, AppendMetadata)],
    ) -> io::Result<Vec<u64>> {
        self.ensure_writable()?;
        validate_target(mailbox, messages)?;
        let total_octets = total_octets(messages)?;
        self.blob_quota.ensure_available(total_octets)?;
        let reservations = reserve_blobs(&self.root, self.next_transaction_id, messages.len())?;
        let transaction_id = match reservations.first() {
            Some(reservation) => reservation.transaction_id,
            None => return Err(invalid_batch("empty APPEND batch")),
        };
        let links = prepare_links(self, mailbox, transaction_id, messages, &reservations)?;
        publish_blobs(self, messages, &reservations)?;
        match self.commit_links(transaction_id, &links) {
            Ok(()) => {}
            Err(error) => {
                self.write_state = WriteState::Poisoned;
                return Err(error);
            }
        }
        self.check_and_poison(AppendStage::BeforeMemoryPublish)?;
        let uids = links
            .iter()
            .map(|(_, record)| record.uid)
            .collect::<Vec<_>>();
        self.publish_links(transaction_id, links, None, None, None)?;
        self.next_transaction_id = reservations
            .last()
            .and_then(|reservation| reservation.transaction_id.checked_add(1))
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?;
        Ok(uids)
    }
}

struct BlobReservation {
    transaction_id: u64,
    name: String,
}

fn validate_target(mailbox: &str, messages: &[(Vec<u8>, AppendMetadata)]) -> io::Result<()> {
    if messages.is_empty() || messages.iter().any(|(bytes, _)| bytes.is_empty()) {
        return Err(invalid_batch("APPEND batch requires nonempty messages"));
    }
    model::validate_mailbox(mailbox)
}

fn total_octets(messages: &[(Vec<u8>, AppendMetadata)]) -> io::Result<usize> {
    messages.iter().try_fold(0_usize, |total, (bytes, _)| {
        total
            .checked_add(bytes.len())
            .ok_or_else(|| invalid_batch("APPEND batch size overflow"))
    })
}

fn reserve_blobs(
    root: &std::path::Path,
    first_id: u64,
    count: usize,
) -> io::Result<Vec<BlobReservation>> {
    let mut next_id = first_id;
    let mut reservations = Vec::with_capacity(count);
    for _ in 0..count {
        let (name, _) = blobs::next_available(root, &mut next_id)?;
        reservations.push(BlobReservation {
            transaction_id: next_id,
            name,
        });
        next_id = next_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?;
    }
    Ok(reservations)
}

fn prepare_links(
    store: &Store,
    mailbox: &str,
    transaction_id: u64,
    messages: &[(Vec<u8>, AppendMetadata)],
    reservations: &[BlobReservation],
) -> io::Result<Vec<(String, model::Record)>> {
    let first_uid = store.next_uids.get(mailbox).copied().unwrap_or(1);
    messages
        .iter()
        .zip(reservations)
        .enumerate()
        .map(|(offset, ((_, metadata), reservation))| {
            let offset = u64::try_from(offset)
                .map_err(|_| io::Error::other("APPEND batch UID count overflow"))?;
            let uid = first_uid
                .checked_add(offset)
                .ok_or_else(|| io::Error::other("uid exhausted"))?;
            uid.checked_add(1)
                .ok_or_else(|| io::Error::other("uid exhausted"))?;
            Ok((
                mailbox.to_owned(),
                model::Record {
                    uid,
                    transaction_id,
                    blob_transaction_id: reservation.transaction_id,
                    name: reservation.name.clone(),
                    flags: metadata.flags,
                    keywords: metadata.keywords.clone(),
                    internal_date: metadata.internal_date,
                },
            ))
        })
        .collect()
}

fn publish_blobs(
    store: &mut Store,
    messages: &[(Vec<u8>, AppendMetadata)],
    reservations: &[BlobReservation],
) -> io::Result<()> {
    for ((bytes, _), reservation) in messages.iter().zip(reservations) {
        if let Err(error) = blobs::publish(&store.root, &reservation.name, bytes) {
            store.write_state = WriteState::Poisoned;
            return Err(error);
        }
        store.blob_quota.record(bytes.len())?;
        store.check_and_poison(AppendStage::BlobPublished)?;
    }
    store.check_and_poison(AppendStage::QueueWritten)?;
    store.check_and_poison(AppendStage::DiagnosticsWritten)?;
    store.check_and_poison(AppendStage::VacationWritten)
}

fn invalid_batch(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
