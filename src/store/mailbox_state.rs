//! Восстановление materialized mailbox index из журналов и commit authority.
//!
//! Все журналы читаются один раз за `O(n)` при старте. Видимые записи затем
//! обслуживаются из `HashMap`, а незавершённые transaction frames резервируют
//! UID и ID, но не попадают клиенту.
use super::commit_log::Committed;
use super::{journal, model};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;

pub(super) struct Recovered {
    pub(super) next_uids: HashMap<String, u64>,
    pub(super) entries: HashMap<String, Vec<model::Record>>,
    pub(super) maximum_transaction_id: u64,
}

/// Восстанавливает UID allocators и видимый materialized view.
pub(super) fn load(
    root: &Path,
    committed: &Committed,
    outbound_transactions: &HashSet<u64>,
) -> io::Result<Recovered> {
    let mut next_uids = HashMap::new();
    let mut entries = HashMap::new();
    let mut maximum_transaction_id = committed.maximum;
    let mut seen_transactions = HashSet::new();
    let mut checked_blobs = HashSet::new();
    let mailbox_files =
        super::file_key::entries(&root.join("mailboxes"), super::file_key::FileKind::Journal)?;
    for (mailbox, path) in mailbox_files {
        let records = journal::records(&mailbox, &path)?;
        let next_uid = match records.last() {
            Some(record) => record
                .uid
                .checked_add(1)
                .ok_or_else(|| io::Error::other("uid exhausted"))?,
            None => 1,
        };
        for record in &records {
            maximum_transaction_id = maximum_transaction_id.max(record.transaction_id);
            seen_transactions.insert(record.transaction_id);
        }
        let visible = records
            .into_iter()
            .filter(|record| committed.ids.contains(&record.transaction_id))
            .collect::<Vec<_>>();
        validate_blobs(root, &visible, &mut checked_blobs)?;
        next_uids.insert(mailbox.clone(), next_uid);
        entries.insert(mailbox, visible);
    }
    for transaction_id in &committed.ids {
        match (
            seen_transactions.contains(transaction_id),
            outbound_transactions.contains(transaction_id),
        ) {
            (true, _) => {}
            (false, true) => {
                fs::metadata(root.join("blobs").join(model::blob_name(*transaction_id)))?;
            }
            (false, false) => return Err(invalid("committed transaction has no mailbox link")),
        }
    }
    Ok(Recovered {
        next_uids,
        entries,
        maximum_transaction_id,
    })
}

fn validate_blobs(
    root: &Path,
    records: &[model::Record],
    checked: &mut HashSet<String>,
) -> io::Result<()> {
    for record in records {
        if checked.insert(record.name.clone()) {
            fs::metadata(root.join("blobs").join(&record.name))?;
        }
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
