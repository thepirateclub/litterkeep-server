//! Публичные Store-операции Sieve без раскрытия внутреннего каталога.

use super::{ScriptInfo, Store};
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
}
