//! Store facade для account-scoped folder catalog и внутренних mailbox keys.

use super::{FolderInfo, Store};
use std::io;

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

    /// Отображает wire-visible имя на безопасный внутренний mailbox key.
    pub fn resolve_folder(&mut self, account: &str, name: &str) -> io::Result<String> {
        self.ensure_writable()?;
        let result = self.folders.resolve(account, name);
        self.finish_folder_operation(result)
    }

    /// Не отравляет writer на ожидаемых policy-отказах до начала записи.
    fn finish_folder_operation<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
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
        drop(store);

        let mut reopened = Store::open(&root).expect("reopen");
        assert_eq!(
            reopened
                .resolve_folder("bob", "Проекты/Почта")
                .expect("resolved"),
            key
        );
        assert_eq!(reopened.folders("bob").expect("folders").len(), 9);
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }
}
