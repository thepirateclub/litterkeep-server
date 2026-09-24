//! Быстрые статусные счётчики mailbox для LIST RETURN STATUS.

use super::{FolderInfo, Store, folders};
use crate::mail_flags::SystemFlag;
use std::fs;
use std::io;

/// Считает mailbox metadata, не загружая тела сообщений из blob-хранилища.
pub(super) fn summaries(
    store: &Store,
    account: &str,
    folders_list: &[FolderInfo],
    include_size: bool,
) -> io::Result<Vec<MailboxStatus>> {
    folders_list
        .iter()
        .map(|folder| summary(store, account, folder, include_size))
        .collect()
}

/// Проецирует один mailbox journal и durable-флаги в STATUS поля.
fn summary(
    store: &Store,
    account: &str,
    folder: &FolderInfo,
    include_size: bool,
) -> io::Result<MailboxStatus> {
    let mailbox = folders::storage_key(account, folder);
    let records = store
        .entries
        .get(&mailbox)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let (unseen, deleted) = records.iter().fold((0_usize, 0_usize), |counts, record| {
        let current = flags(store, &mailbox, record.uid, record.flags);
        let unseen = counts.0 + usize::from(!current.contains(SystemFlag::Seen));
        let deleted = counts.1 + usize::from(current.contains(SystemFlag::Deleted));
        (unseen, deleted)
    });
    let size = match include_size {
        true => Some(mailbox_size(store, records)?),
        false => None,
    };
    Ok(MailboxStatus {
        name: folder.name.clone(),
        messages: records.len(),
        uid_next: store.next_uids.get(&mailbox).copied().unwrap_or(1),
        uid_validity: 1,
        unseen,
        deleted,
        size,
    })
}

/// Складывает октетные размеры blob-ов с переполнением, а не wraparound.
fn mailbox_size(store: &Store, records: &[super::model::Record]) -> io::Result<u64> {
    records.iter().try_fold(0_u64, |total, record| {
        let path = store.root.join("blobs").join(&record.name);
        let length = fs::metadata(path)?.len();
        total
            .checked_add(length)
            .ok_or_else(|| io::Error::other("mailbox SIZE counter overflow"))
    })
}

/// Возвращает актуальные flags после STORE, сохраняя исходные при отсутствии override.
fn flags(
    store: &Store,
    mailbox: &str,
    uid: u64,
    initial: crate::mail_flags::MessageFlags,
) -> crate::mail_flags::MessageFlags {
    store
        .flags
        .get(mailbox)
        .and_then(|values| values.get(&uid))
        .map(|stored| stored.system)
        .unwrap_or(initial)
}

/// Согласованные счётчики ответа STATUS без payload-байтов.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MailboxStatus {
    pub name: String,
    pub messages: usize,
    pub uid_next: u64,
    pub uid_validity: u32,
    pub unseen: usize,
    pub deleted: usize,
    pub size: Option<u64>,
}
