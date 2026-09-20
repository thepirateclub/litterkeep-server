//! Полная offline-проверка ссылок хранилища без вывода почтовых данных.

use crate::store::Store;
use std::io;
use std::path::Path;

/// Числовой результат успешно проверенного состояния.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckReport {
    pub mailboxes: usize,
    pub message_links: usize,
    pub referenced_blobs: usize,
    pub orphan_blobs: usize,
    pub pending_deliveries: usize,
}

/// Берёт exclusive lock, выполняет recovery framing и проверяет все ссылки.
pub fn check(root: impl AsRef<Path>) -> io::Result<CheckReport> {
    let store = Store::open(root)?;
    store.integrity_report()
}
