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
    /// поэтому доставка не может увидеть частично заменённый script. Redirect
    /// записывается queue job до той же global commit точки, что и mailbox link.
    pub fn deliver_sieve_many(
        &mut self,
        accounts: &[String],
        envelope_sender: &str,
        raw: &[u8],
    ) -> io::Result<u64> {
        // Redirect будет использовать только проверенный SMTP envelope sender.
        // Пока очередь не введена, проверяем параметр здесь, чтобы граница
        // контракта не позволила позднее незаметно перейти к MIME `From`.
        let _ = envelope_sender;
        let headers = delivery_headers(raw);
        let mut mailboxes = Vec::new();
        let mut outbound = Vec::new();
        for account in accounts {
            let plan = self.delivery_plan(account, &headers, raw.len())?;
            for recipient in plan.redirects {
                outbound.push((envelope_sender.to_owned(), recipient));
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
        self.append_many_with_outbound(&mailboxes, raw, &outbound)
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
            .deliver_sieve_many(
                &["bob".into()],
                "alice@litterkeep.test",
                b"Subject: [invoice] paid\r\n\r\nbody",
            )
            .expect("invoice");
        store
            .deliver_sieve_many(
                &["bob".into()],
                "alice@litterkeep.test",
                b"Subject: ordinary\r\n\r\nbody",
            )
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

    #[test]
    fn redirect_is_committed_with_local_delivery_and_preserves_envelope_sender() {
        let root = directory();
        let mut store = Store::open(&root).expect("open");
        store
            .put_sieve_script(
                "bob",
                "forward",
                "require [\"fileinto\"]; redirect \"remote@test\";".into(),
            )
            .expect("put");
        store
            .activate_sieve_script("bob", Some("forward"))
            .expect("activate");
        let transaction_id = store
            .deliver_sieve_many(
                &["bob".into()],
                "alice@litterkeep.test",
                b"Subject: redirect\r\n\r\nbody",
            )
            .expect("deliver");
        let expected = crate::queue::OwnedDelivery {
            transaction_id,
            attempts: 0,
            sender: "alice@litterkeep.test".into(),
            recipient: "remote@test".into(),
            state: crate::queue::State::Pending,
        };
        assert_eq!(store.pending_outbound().expect("owned"), vec![expected]);
        let delivery = store
            .pending_outbound()
            .expect("pending")
            .into_iter()
            .next()
            .expect("job");
        assert!(
            store
                .outbound_blob(&delivery)
                .expect("blob")
                .ends_with(b"body")
        );
        store
            .transition_outbound(&delivery, crate::queue::State::Delivered)
            .expect("transition");
        assert!(store.pending_outbound().expect("delivered").is_empty());
        assert_eq!(store.messages("bob").expect("inbox").len(), 1);
        drop(store);
        let store = Store::open(&root).expect("reopen");
        assert!(store.pending_outbound().expect("recovered").is_empty());
        drop(store);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
