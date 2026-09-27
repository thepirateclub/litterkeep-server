//! Версионный каталог Sieve scripts, изменяемый только владельцем Store.
//!
//! Каждая account получает один атомарно заменяемый snapshot. Ошибка parser-а,
//! квоты или fsync происходит до rename, поэтому действующий script сохраняется.

#[path = "sieve_script_codec.rs"]
mod codec;

use super::folders::validate_account;
use crate::sieve;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_SCRIPTS: usize = 16;
const MAX_SCRIPT_BYTES: usize = 65_536;
const MAX_ACCOUNT_BYTES: usize = 262_144;

/// Метаданные script без исходника, безопасные для протокольного ответа.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptInfo {
    pub name: String,
    pub version: u64,
    pub active: bool,
}

#[derive(Debug)]
pub(super) struct SieveCatalog {
    root: PathBuf,
    accounts: HashMap<String, AccountScripts>,
}

#[derive(Clone, Debug, Default)]
struct AccountScripts {
    next_version: u64,
    active: Option<String>,
    scripts: BTreeMap<String, ScriptRecord>,
}

#[derive(Clone, Debug)]
struct ScriptRecord {
    version: u64,
    source: String,
}

impl SieveCatalog {
    pub(super) fn load(root: &Path) -> io::Result<Self> {
        let directory = root.join("sieve");
        fs::create_dir_all(&directory)?;
        let mut accounts = HashMap::new();
        let files = super::file_key::entries(&directory, super::file_key::FileKind::SieveState)?;
        for (account, path) in files {
            validate_account(&account)?;
            accounts.insert(account, codec::decode(&fs::read_to_string(path)?)?);
        }
        Ok(Self {
            root: directory,
            accounts,
        })
    }

    pub(super) fn put(
        &mut self,
        account: &str,
        name: &str,
        source: String,
    ) -> io::Result<ScriptInfo> {
        validate_account(account)?;
        valid_name(name)?;
        sieve::parse(&source).map_err(invalid)?;
        if source.len() > MAX_SCRIPT_BYTES {
            return Err(quota_exceeded("sieve script quota exceeded"));
        }
        let candidate = self.candidate(account, |state| {
            if let (false, true) = (
                state.scripts.contains_key(name),
                state.scripts.len() >= MAX_SCRIPTS,
            ) {
                return Err(quota_exceeded("sieve script count exceeded"));
            }
            state.next_version = state
                .next_version
                .checked_add(1)
                .ok_or_else(|| invalid("sieve version exhausted"))?;
            state.scripts.insert(
                name.to_owned(),
                ScriptRecord {
                    version: state.next_version,
                    source,
                },
            );
            Ok(())
        })?;
        let info = info(&candidate, name)?;
        self.publish(account, candidate)?;
        Ok(info)
    }

    pub(super) fn activate(&mut self, account: &str, name: Option<&str>) -> io::Result<()> {
        let candidate = self.candidate(account, |state| {
            match name {
                Some(value) if state.scripts.contains_key(value) => {
                    state.active = Some(value.to_owned())
                }
                Some(_) => return Err(missing()),
                None => state.active = None,
            }
            Ok(())
        })?;
        self.publish(account, candidate)
    }

    pub(super) fn delete(&mut self, account: &str, name: &str) -> io::Result<()> {
        let candidate = self.candidate(account, |state| {
            if state.active.as_deref() == Some(name) {
                return Err(invalid("cannot delete active sieve script"));
            }
            match state.scripts.remove(name) {
                Some(_) => Ok(()),
                None => Err(missing()),
            }
        })?;
        self.publish(account, candidate)
    }

    pub(super) fn rename(&mut self, account: &str, old: &str, new: &str) -> io::Result<()> {
        valid_name(new)?;
        let candidate = self.candidate(account, |state| {
            if state.scripts.contains_key(new) {
                return Err(invalid("sieve script already exists"));
            }
            let record = state.scripts.remove(old).ok_or_else(missing)?;
            if state.active.as_deref() == Some(old) {
                state.active = Some(new.to_owned());
            }
            state.scripts.insert(new.to_owned(), record);
            Ok(())
        })?;
        self.publish(account, candidate)
    }

    pub(super) fn have_space(&self, account: &str, name: &str, size: u64) -> io::Result<bool> {
        validate_account(account)?;
        valid_name(name)?;
        match usize::try_from(size) {
            Ok(size) if size <= MAX_SCRIPT_BYTES => Ok(true),
            Ok(_) | Err(_) => Ok(false),
        }
    }

    pub(super) fn source(&self, account: &str, name: &str) -> io::Result<(u64, String)> {
        let state = self.accounts.get(account).ok_or_else(missing)?;
        let record = state.scripts.get(name).ok_or_else(missing)?;
        Ok((record.version, record.source.clone()))
    }

    pub(super) fn active(&self, account: &str) -> io::Result<Option<(u64, String)>> {
        let Some(state) = self.accounts.get(account) else {
            return Ok(None);
        };
        match state.active.as_deref() {
            Some(name) => self.source(account, name).map(Some),
            None => Ok(None),
        }
    }

    pub(super) fn list(&self, account: &str) -> Vec<ScriptInfo> {
        let Some(state) = self.accounts.get(account) else {
            return Vec::new();
        };
        state
            .scripts
            .keys()
            .filter_map(|name| info(state, name).ok())
            .collect()
    }

    fn candidate(
        &self,
        account: &str,
        change: impl FnOnce(&mut AccountScripts) -> io::Result<()>,
    ) -> io::Result<AccountScripts> {
        let mut state = self.accounts.get(account).cloned().unwrap_or_default();
        change(&mut state)?;
        if codec::encoded(&state).len() > MAX_ACCOUNT_BYTES {
            return Err(quota_exceeded("sieve account quota exceeded"));
        }
        Ok(state)
    }

    fn publish(&mut self, account: &str, state: AccountScripts) -> io::Result<()> {
        let temporary = super::file_key::prepare(
            &self.root,
            account,
            super::file_key::FileKind::SieveTemporary,
        )?;
        let target =
            super::file_key::prepare(&self.root, account, super::file_key::FileKind::SieveState)?;
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(codec::encoded(&state).as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &target)?;
        let directory = target.parent().unwrap_or(&self.root);
        super::file_key::sync_key_directory(&self.root, directory)?;
        self.accounts.insert(account.to_owned(), state);
        Ok(())
    }
}

fn info(state: &AccountScripts, name: &str) -> io::Result<ScriptInfo> {
    let record = state.scripts.get(name).ok_or_else(missing)?;
    Ok(ScriptInfo {
        name: name.to_owned(),
        version: record.version,
        active: state.active.as_deref() == Some(name),
    })
}

fn valid_name(name: &str) -> io::Result<()> {
    if name.is_empty() {
        return Err(invalid("unsafe sieve script name"));
    }
    if name.len() > 120 {
        return Err(invalid("unsafe sieve script name"));
    }
    if name.contains('/') {
        return Err(invalid("unsafe sieve script name"));
    }
    Ok(())
}
fn missing() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "sieve script missing")
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn quota_exceeded(message: &'static str) -> io::Error {
    io::Error::other(QuotaExceeded(message))
}

#[cfg(any(test, target_os = "linux"))]
pub(super) fn is_quota_error(error: &io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|source| source.is::<QuotaExceeded>())
}

#[cfg(all(test, target_os = "linux"))]
pub(super) fn quota_test_error() -> io::Error {
    quota_exceeded("test quota")
}

#[derive(Debug)]
struct QuotaExceeded(&'static str);

impl std::fmt::Display for QuotaExceeded {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for QuotaExceeded {}
