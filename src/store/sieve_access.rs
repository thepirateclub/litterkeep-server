//! Публичные Store-операции Sieve без раскрытия внутреннего каталога.

use super::{ScriptInfo, Store};
use crate::sieve;
use std::io;

impl Store {
    /// Валидирует и атомарно публикует новую версию named script.
    pub fn put_sieve_script(
        &mut self,
        account: &str,
        name: &str,
        source: String,
    ) -> io::Result<ScriptInfo> {
        self.sieve_scripts.put(account, name, source)
    }

    /// Меняет active script только после durable snapshot replacement.
    pub fn activate_sieve_script(&mut self, account: &str, name: Option<&str>) -> io::Result<()> {
        self.sieve_scripts.activate(account, name)
    }

    /// Возвращает неизменяемую версию script для MANAGESIEVE GETSCRIPT.
    pub fn sieve_script(&self, account: &str, name: &str) -> io::Result<(u64, String)> {
        self.sieve_scripts.source(account, name)
    }

    /// Фиксирует active source и его version перед delivery transaction.
    pub fn active_sieve_script(&self, account: &str) -> io::Result<Option<(u64, String)>> {
        self.sieve_scripts.active(account)
    }

    /// Отдаёт только метаданные scripts текущего владельца account.
    pub fn sieve_scripts(&self, account: &str) -> Vec<ScriptInfo> {
        self.sieve_scripts.list(account)
    }

    /// Удаляет только inactive script, сохраняя явно выбранную active версию.
    pub fn delete_sieve_script(&mut self, account: &str, name: &str) -> io::Result<()> {
        self.sieve_scripts.delete(account, name)
    }

    /// Переименовывает script atomically и переносит active reference при нужде.
    pub fn rename_sieve_script(&mut self, account: &str, old: &str, new: &str) -> io::Result<()> {
        self.sieve_scripts.rename(account, old, new)
    }

    /// Проверяет доступное место без изменения versioned state.
    pub fn sieve_have_space(&self, account: &str, name: &str, size: u64) -> io::Result<bool> {
        self.sieve_scripts.have_space(account, name, size)
    }

    /// Проверяет source тем же parser/validator, что и PUTSCRIPT.
    pub fn check_sieve_script(&self, source: &str) -> io::Result<()> {
        sieve::parse(source).map(|_| ()).map_err(io::Error::other)
    }

    /// Строит планы всех recipients до первой записи mailbox journal.
    ///
    /// Active source читается вместе с его версией у единственного Store owner,
    /// поэтому доставка не может увидеть частично заменённый script. Пока
    /// redirect не подключён к исходящей очереди, он является delivery error,
    /// а не незаметно потерянным внешним письмом.
    pub fn deliver_sieve_many(&mut self, accounts: &[String], raw: &[u8]) -> io::Result<u64> {
        let headers = delivery_headers(raw);
        let mut mailboxes = Vec::new();
        for account in accounts {
            let plan = self.delivery_plan(account, &headers, raw.len())?;
            if !plan.redirects.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "sieve redirect queue is unavailable",
                ));
            }
            if plan.keep {
                mailboxes.push(self.resolve_folder(account, "INBOX")?);
            }
            for folder in plan.folders {
                mailboxes.push(self.resolve_folder(account, &folder)?);
            }
        }
        if mailboxes.is_empty() {
            return Ok(0);
        }
        self.append_many(&mailboxes, raw)
    }

    fn delivery_plan(
        &self,
        account: &str,
        headers: &[(String, String)],
        message_size: usize,
    ) -> io::Result<sieve::DeliveryPlan> {
        let Some((_, source)) = self.active_sieve_script(account)? else {
            return Ok(sieve::DeliveryPlan {
                keep: true,
                discard: false,
                folders: Vec::new(),
                redirects: Vec::new(),
            });
        };
        let script = sieve::parse(&source).map_err(io::Error::other)?;
        sieve::execute(&script, headers, message_size).map_err(io::Error::other)
    }
}

/// Malformed MIME не отменяет уже принятую SMTP-доставку: правило не совпадает.
fn delivery_headers(raw: &[u8]) -> Vec<(String, String)> {
    match crate::mime::headers(raw) {
        Ok((headers, _)) => headers.into_iter().collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn directory() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "litterkeep-sieve-delivery-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn invoice_plan_delivers_to_invoices_and_default_goes_to_inbox() {
        let root = directory();
        let mut store = Store::open(&root).expect("open");
        store
            .put_sieve_script("bob", "invoices", "require [\"fileinto\"]; if header :contains \"subject\" \"[invoice]\" { fileinto \"Invoices\"; stop; }".into())
            .expect("put");
        store
            .activate_sieve_script("bob", Some("invoices"))
            .expect("activate");
        store
            .deliver_sieve_many(&["bob".into()], b"Subject: [invoice] paid\r\n\r\nbody")
            .expect("invoice");
        store
            .deliver_sieve_many(&["bob".into()], b"Subject: ordinary\r\n\r\nbody")
            .expect("ordinary");
        assert_eq!(
            store
                .snapshot_folder("bob", "Invoices")
                .expect("invoices")
                .snapshot
                .messages
                .len(),
            1
        );
        assert_eq!(
            store
                .snapshot_folder("bob", "INBOX")
                .expect("inbox")
                .snapshot
                .messages
                .len(),
            1
        );
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
