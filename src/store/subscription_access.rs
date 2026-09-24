//! Store facade для подписок и LSUB-проекции каталога.

use super::{Store, SubscribedFolder, SubscriptionSnapshot};
use std::io;

impl Store {
    /// Идемпотентно меняет подписку после долговечной journal-фиксации.
    pub fn set_subscription(
        &mut self,
        account: &str,
        name: &str,
        subscribed: bool,
    ) -> io::Result<()> {
        self.ensure_writable()?;
        let result = self.subscriptions.set(account, name, subscribed);
        self.finish_folder_operation(result)
    }

    /// Возвращает подписанные имена, включая удалённые mailbox как `\Noselect`.
    pub fn subscriptions(&mut self, account: &str) -> io::Result<Vec<SubscribedFolder>> {
        self.subscription_snapshot(account)
            .map(|snapshot| snapshot.subscriptions)
    }

    /// Возвращает подписки и каталог одной операцией единственного Store owner-а.
    pub fn subscription_snapshot(&mut self, account: &str) -> io::Result<SubscriptionSnapshot> {
        self.subscription_snapshot_with_status(account, false, false)
    }

    /// Включает быстрые счётчики и, по запросу, размеры в тот же Store snapshot.
    pub fn subscription_snapshot_with_status(
        &mut self,
        account: &str,
        include_status: bool,
        include_size: bool,
    ) -> io::Result<SubscriptionSnapshot> {
        self.ensure_writable()?;
        let names = self.subscriptions.list(account)?;
        let folders = self.folders.list(account)?;
        let subscriptions = names
            .into_iter()
            .map(|name| {
                let existing = folders.iter().find(|folder| match folder.special_use {
                    Some(super::SpecialUse::Inbox) => name.eq_ignore_ascii_case("INBOX"),
                    _ => folder.name == name,
                });
                SubscribedFolder {
                    name,
                    exists: existing.is_some(),
                    special_use: existing.and_then(|folder| folder.special_use),
                }
            })
            .collect();
        let statuses = match include_status {
            true => super::folder_status::summaries(self, account, &folders, include_size)?,
            false => Vec::new(),
        };
        Ok(SubscriptionSnapshot {
            subscriptions,
            folders,
            statuses,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn subscriptions_are_idempotent_and_survive_restart() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-subscriptions-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        store
            .set_subscription("bob", "INBOX", true)
            .expect("inbox subscription");
        store
            .set_subscription("bob", "Missing", true)
            .expect("missing subscription");
        store
            .set_subscription("bob", "Missing", true)
            .expect("idempotent subscription");
        drop(store);

        let journal = root.join("subscriptions").join("bob.journal");
        let committed_length = fs::metadata(&journal).expect("metadata").len();
        let mut file = OpenOptions::new()
            .append(true)
            .open(&journal)
            .expect("journal");
        file.write_all(b"LKSB\0\x01\0\0\x07Missing")
            .expect("partial unsubscribe");
        file.sync_all().expect("sync partial");
        drop(file);

        let mut reopened = Store::open(&root).expect("reopen");
        assert_eq!(
            fs::metadata(&journal).expect("trimmed metadata").len(),
            committed_length
        );
        let subscriptions = reopened.subscriptions("bob").expect("subscriptions");
        assert_eq!(subscriptions.len(), 2);
        assert!(subscriptions.iter().any(|folder| !folder.exists));
        reopened
            .set_subscription("bob", "Missing", false)
            .expect("unsubscribe");
        assert_eq!(reopened.subscriptions("bob").expect("remaining").len(), 1);
        drop(reopened);
        fs::remove_dir_all(root).expect("cleanup");
    }

    /// LIST status использует journal metadata и считает флаги и размеры без чтения blob.
    #[test]
    fn status_snapshot_reports_counts_uid_and_exact_blob_octets() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-list-status-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        let seen_deleted = b"Subject: gone\r\n\r\nbody";
        store
            .append_with_metadata(
                "bob",
                seen_deleted,
                crate::store::AppendMetadata {
                    flags: crate::mail_flags::MessageFlags::from_flags([
                        crate::mail_flags::SystemFlag::Seen,
                        crate::mail_flags::SystemFlag::Deleted,
                    ]),
                    ..crate::store::AppendMetadata::default()
                },
            )
            .expect("append deleted");
        let unseen = b"Subject: new\r\n\r\ntext";
        store.append("bob", unseen).expect("append unseen");

        let snapshot = store
            .subscription_snapshot_with_status("bob", true, true)
            .expect("status snapshot");
        let inbox = snapshot
            .statuses
            .iter()
            .find(|status| status.name == "INBOX")
            .expect("inbox status");
        assert_eq!(inbox.messages, 2);
        assert_eq!(inbox.uid_next, 3);
        assert_eq!(inbox.uid_validity, 1);
        assert_eq!(inbox.unseen, 1);
        assert_eq!(inbox.deleted, 1);
        assert_eq!(inbox.size, Some((seen_deleted.len() + unseen.len()) as u64));
        let without_status = store
            .subscription_snapshot_with_status("bob", false, false)
            .expect("folder-only snapshot");
        assert!(without_status.statuses.is_empty());
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
