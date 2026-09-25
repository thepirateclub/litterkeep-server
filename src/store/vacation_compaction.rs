//! Безопасное уплотнение и восстановление журнала suppression RFC 5230.

use super::vacation_suppression::{
    MIN_TRACKED_RESPONSES, Record, VacationKey, encode_record, path, read_complete,
};
use crate::store::durability;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const TEMPORARY_NAME: &str = "vacation-history.compact.tmp";
const BACKUP_NAME: &str = "vacation-history.compact.bak";

/// Убирает истёкшие/повторные claims, а при лимите удаляет старейшие первые.
pub(super) fn compact(
    root: &Path,
    committed: &HashSet<u64>,
    now_epoch: u64,
    incoming_bytes: u64,
    maximum_bytes: u64,
) -> io::Result<()> {
    let Some(snapshot_limit) = maximum_bytes.checked_sub(incoming_bytes) else {
        return Err(full());
    };
    let mut latest = HashMap::<VacationKey, (usize, Record)>::new();
    for (position, record) in read_complete(root)?.into_iter().enumerate() {
        if committed.contains(&record.transaction_id) {
            latest.insert(record.key.clone(), (position, record));
        }
    }
    let mut active: Vec<_> = latest
        .into_values()
        .filter(|(_, record)| record.expires_at > now_epoch)
        .collect();
    active.sort_by_key(|(position, _)| *position);
    let mut lines = encode_ordered(active);
    let mut encoded_bytes = total_bytes(&lines)?;
    while encoded_bytes > snapshot_limit && lines.len() > MIN_TRACKED_RESPONSES {
        match lines.pop_front() {
            Some(oldest) => {
                let length = u64::try_from(oldest.len()).map_err(|_| full())?;
                encoded_bytes = encoded_bytes.saturating_sub(length);
            }
            None => break,
        }
    }
    if encoded_bytes > snapshot_limit {
        return Err(full());
    }
    replace_snapshot(root, &mut lines)
}

fn encode_ordered(records: Vec<(usize, Record)>) -> VecDeque<Vec<u8>> {
    records
        .into_iter()
        .map(|(_, record)| encode_record(&record))
        .collect()
}

fn total_bytes(lines: &VecDeque<Vec<u8>>) -> io::Result<u64> {
    lines.iter().try_fold(0_u64, |total, line| {
        let length = u64::try_from(line.len()).map_err(|_| full())?;
        total.checked_add(length).ok_or_else(full)
    })
}

fn replace_snapshot(root: &Path, lines: &mut VecDeque<Vec<u8>>) -> io::Result<()> {
    recover_replacement(root)?;
    let target = path(root);
    let temporary = sibling(root, TEMPORARY_NAME);
    let backup = sibling(root, BACKUP_NAME);
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    for line in lines {
        output.write_all(line)?;
    }
    output.sync_all()?;
    drop(output);
    if target.try_exists()? {
        fs::rename(&target, &backup)?;
        durability::sync_directory(&root.join("journal"))?;
    }
    fs::rename(&temporary, &target)?;
    durability::sync_directory(&root.join("journal"))?;
    remove_if_present(&backup)?;
    durability::sync_directory(&root.join("journal"))
}

/// Завершает прерванную двухпереименовательную замену до чтения Store.
pub(super) fn recover_replacement(root: &Path) -> io::Result<()> {
    let target = path(root);
    let temporary = sibling(root, TEMPORARY_NAME);
    let backup = sibling(root, BACKUP_NAME);
    let has_target = target.try_exists()?;
    let has_backup = backup.try_exists()?;
    let mut changed = false;
    match (has_target, has_backup) {
        (true, true) => {
            fs::remove_file(&backup)?;
            changed = true;
        }
        (false, true) => {
            fs::rename(&backup, &target)?;
            changed = true;
        }
        (true, false) | (false, false) => {}
    }
    if temporary.try_exists()? {
        fs::remove_file(&temporary)?;
        changed = true;
    }
    match changed {
        true => durability::sync_directory(&root.join("journal")),
        false => Ok(()),
    }
}

fn sibling(root: &Path, name: &str) -> PathBuf {
    root.join("journal").join(name)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match path.try_exists()? {
        true => fs::remove_file(path),
        false => Ok(()),
    }
}

fn full() -> io::Error {
    io::Error::new(io::ErrorKind::StorageFull, "vacation history full")
}
