//! Публичные Store-операции Sieve без раскрытия внутреннего каталога.

use super::{ScriptInfo, SieveDiagnostic, Store, sieve_diagnostics};
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

    /// Возвращает долговечные runtime-ошибки только committed доставок account.
    pub fn sieve_diagnostics(&self, account: &str) -> io::Result<Vec<SieveDiagnostic>> {
        sieve_diagnostics::load(&self.root, &self.committed_transactions, account)
    }
}

#[cfg(test)]
#[path = "sieve_access_tests.rs"]
mod tests;
