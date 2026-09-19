//! Store facade для account-scoped folder catalog и внутренних mailbox keys.

use super::{FolderInfo, Store};
use std::io;

/// Snapshot вместе с внутренним ключом выбранной папки.
#[derive(Debug)]
pub struct ResolvedMailbox {
    pub key: String,
    pub snapshot: super::MailboxSnapshot,
}

impl Store {
    /// Возвращает стандартные и пользовательские папки в устойчивом порядке ID.
    pub fn folders(&mut self, account: &str) -> io::Result<Vec<FolderInfo>> {
        self.ensure_writable()?;
        let result = self.folders.list(account);
        self.finish_folder_operation(result)
    }

    /// Создаёт папку, не используя её имя как компонент filesystem path.
    pub fn create_folder(&mut self, account: &str, name: &str) -> io::Result<FolderInfo> {
        self.ensure_writable()?;
        let result = self.folders.create(account, name);
        self.finish_folder_operation(result)
    }
    pub fn create_special_folder(
        &mut self,
        account: &str,
        name: &str,
        special: super::SpecialUse,
    ) -> io::Result<FolderInfo> {
        self.ensure_writable()?;
        let result = self.folders.create_special(account, name, special);
        self.finish_folder_operation(result)
    }

    /// Переименовывает папку и потомков после долговечной journal-фиксации.
    pub fn rename_folder(&mut self, account: &str, old: &str, new: &str) -> io::Result<()> {
        self.ensure_writable()?;
        let result = self.folders.rename(account, old, new);
        self.finish_folder_operation(result)
    }

    /// Удаляет leaf-папку, не разрешая повторное использование её ID.
    pub fn delete_folder(&mut self, account: &str, name: &str) -> io::Result<()> {
        self.ensure_writable()?;
        let result = self.folders.delete(account, name);
        self.finish_folder_operation(result)
    }

    /// Отображает wire-visible имя на безопасный внутренний mailbox key.
    pub fn resolve_folder(&mut self, account: &str, name: &str) -> io::Result<String> {
        self.ensure_writable()?;
        let result = self.folders.resolve(account, name);
        self.finish_folder_operation(result)
    }

    /// Разрешает имя и читает snapshot одним последовательным действием writer-а.
    pub fn snapshot_folder(&mut self, account: &str, name: &str) -> io::Result<ResolvedMailbox> {
        let key = self.resolve_folder(account, name)?;
        let snapshot = self.snapshot(&key)?;
        Ok(ResolvedMailbox { key, snapshot })
    }

    /// Не отравляет writer на ожидаемых policy-отказах до начала записи.
    pub(super) fn finish_folder_operation<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::AlreadyExists
                        | io::ErrorKind::InvalidInput
                        | io::ErrorKind::NotFound
                ) =>
            {
                Err(error)
            }
            Err(error) => {
                self.write_state = super::WriteState::Poisoned;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn defaults_and_unicode_folder_survive_restart() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-folders-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        assert_eq!(store.folders("bob").expect("defaults").len(), 8);
        let created = store.create_folder("bob", "Проекты/Почта").expect("create");
        let key = store.resolve_folder("bob", &created.name).expect("key");
        assert!(!key.contains("Проекты"));
        store
            .create_folder("alice", "Проекты/Почта")
            .expect("isolated create");
        let alice_key = store
            .resolve_folder("alice", "Проекты/Почта")
            .expect("isolated key");
        assert_ne!(key, alice_key);
        store.append(&key, b"named folder message").expect("append");
        store
            .create_folder("bob", "Проекты/Почта/2026")
            .expect("child");
        store
            .rename_folder("bob", "Проекты/Почта", "Работа/Почта")
            .expect("rename tree");
        assert!(store.resolve_folder("bob", "Работа/Почта/2026").is_ok());
        assert!(store.delete_folder("bob", "Работа/Почта").is_err());
        store
            .delete_folder("bob", "Работа/Почта/2026")
            .expect("delete leaf");
        drop(store);

        let mut reopened = Store::open(&root).expect("reopen");
        assert_eq!(
            reopened
                .resolve_folder("bob", "Работа/Почта")
                .expect("resolved"),
            key
        );
        let selected = reopened
            .snapshot_folder("bob", "Работа/Почта")
            .expect("snapshot");
        assert_eq!(selected.snapshot.messages.len(), 1);
        assert_eq!(reopened.folders("bob").expect("folders").len(), 9);
        let recreated = reopened
            .create_folder("bob", "Работа/Почта/2026")
            .expect("recreate");
        assert!(recreated.id > created.id);
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn special_use_folder_survives_journal_replay() {
        let root = std::env::temp_dir().join(format!(
            "litterkeep-special-folder-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let mut store = Store::open(&root).expect("store");
        store
            .create_special_folder("bob", "Archive2", super::super::SpecialUse::Archive)
            .expect("create");
        drop(store);
        let mut reopened = Store::open(&root).expect("reopen");
        assert!(
            reopened
                .folders("bob")
                .expect("folders")
                .iter()
                .any(|folder| folder.name == "Archive2"
                    && folder.special_use == Some(super::super::SpecialUse::Archive))
        );
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }
}
