//! Версионный каталог Sieve scripts, изменяемый только владельцем Store.
//!
//! Каждая account получает один атомарно заменяемый snapshot. Ошибка parser-а,
//! квоты или fsync происходит до rename, поэтому действующий script сохраняется.

use super::{durability, folders::validate_account};
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
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("state") {
                continue;
            }
            let Some(account) = path.file_stem().and_then(|value| value.to_str()) else {
                return Err(invalid("non-utf8 sieve account"));
            };
            validate_account(account)?;
            accounts.insert(account.to_owned(), decode(&fs::read_to_string(path)?)?);
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
            return Err(invalid("sieve script quota exceeded"));
        }
        let candidate = self.candidate(account, |state| {
            if let (false, true) = (
                state.scripts.contains_key(name),
                state.scripts.len() >= MAX_SCRIPTS,
            ) {
                return Err(invalid("sieve script count exceeded"));
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
        if encoded(&state).len() > MAX_ACCOUNT_BYTES {
            return Err(invalid("sieve account quota exceeded"));
        }
        Ok(state)
    }

    fn publish(&mut self, account: &str, state: AccountScripts) -> io::Result<()> {
        let temporary = self.root.join(format!(".{account}.tmp"));
        let target = self.root.join(format!("{account}.state"));
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(encoded(&state).as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, target)?;
        durability::sync_directory(&self.root)?;
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

fn encoded(state: &AccountScripts) -> String {
    let mut output = format!(
        "next:{}\nactive:{}\n",
        state.next_version,
        state.active.as_deref().map(hex).unwrap_or_default()
    );
    for (name, record) in &state.scripts {
        output.push_str(&format!(
            "script:{}:{}:{}\n",
            hex(name),
            record.version,
            hex(&record.source)
        ));
    }
    output
}

fn decode(source: &str) -> io::Result<AccountScripts> {
    let mut state = AccountScripts::default();
    for line in source.lines() {
        if let Some(value) = line.strip_prefix("next:") {
            state.next_version = value
                .parse()
                .map_err(|_| invalid("bad sieve next version"))?;
            continue;
        }
        if let Some(value) = line.strip_prefix("active:") {
            state.active = match value.is_empty() {
                true => None,
                false => Some(unhex(value)?),
            };
            continue;
        }
        let Some(value) = line.strip_prefix("script:") else {
            return Err(invalid("bad sieve frame"));
        };
        let mut fields = value.split(':');
        let (Some(name), Some(version), Some(body), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(invalid("bad sieve script frame"));
        };
        let name = unhex(name)?;
        valid_name(&name)?;
        let version = version.parse().map_err(|_| invalid("bad sieve version"))?;
        let body = unhex(body)?;
        sieve::parse(&body).map_err(invalid)?;
        state.scripts.insert(
            name,
            ScriptRecord {
                version,
                source: body,
            },
        );
    }
    Ok(state)
}

fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn unhex(value: &str) -> io::Result<String> {
    let mut output = Vec::new();
    let mut bytes = value.as_bytes().chunks_exact(2);
    for pair in &mut bytes {
        let text = std::str::from_utf8(pair).map_err(|_| invalid("non-utf8 sieve hex"))?;
        output.push(u8::from_str_radix(text, 16).map_err(|_| invalid("bad sieve hex"))?);
    }
    if !bytes.remainder().is_empty() {
        return Err(invalid("odd sieve hex"));
    }
    String::from_utf8(output).map_err(|_| invalid("non-utf8 sieve source"))
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
