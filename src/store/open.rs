//! Открытие store и восстановление всех долговечных индексов.

use super::*;

impl Store {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_with_quota(root.as_ref(), DEFAULT_BLOB_QUOTA)
    }

    /// Открывает store с явным budget для граничных тестов квоты.
    pub(super) fn open_with_quota(root: &Path, quota_limit: u64) -> io::Result<Self> {
        let root = root.to_path_buf();
        fs::create_dir_all(&root)?;
        format::ensure(&root)?;
        for directory in [
            "blobs",
            "mailboxes",
            "journal",
            "folders",
            "subscriptions",
            "sieve",
        ] {
            fs::create_dir_all(root.join(directory))?;
        }
        durability::sync_directory(&root)?;
        let lock = open_writer_lock(&root.join("writer.lock"))?;
        recovery::recover(&root)?;
        let committed = commit_log::committed(&root.join("journal/transactions.log"))?;
        let recovered = mailbox_state::load(&root, &committed)?;
        let mut entries = recovered.entries;
        let flags = flags::load(&root, &entries)?;
        expunges::load(&root, &mut entries, &committed)?;
        let recent = recent::load(&root, &committed)?;
        let blob_quota = Quota::load(&root.join("blobs"), quota_limit)?;
        let folders = folders::FolderCatalog::load(&root)?;
        let subscriptions = subscriptions::SubscriptionCatalog::load(&root)?;
        let sieve_scripts = sieve_scripts::SieveCatalog::load(&root)?;
        let queue = crate::queue::Queue::open(root.join("queue"))?;
        let next_transaction_id = recovered
            .maximum_transaction_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?;
        Ok(Self {
            root,
            _writer_lock: lock,
            next_uids: recovered.next_uids,
            entries,
            flags,
            recent,
            folders,
            subscriptions,
            sieve_scripts,
            queue,
            next_transaction_id,
            blob_quota,
            write_state: WriteState::Writable,
            #[cfg(test)]
            failpoint: None,
        })
    }
}
