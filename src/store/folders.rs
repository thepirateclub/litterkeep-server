//! Долговечный каталог папок и отображение пользовательских имён на внутренние ID.

use super::folder_journal;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "folder_name_tests.rs"]
mod name_tests;

#[path = "folder_names.rs"]
mod names;

pub(super) const MAX_NAME: usize = 120;

/// Семантика стандартной папки не выводится из локализованного имени.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpecialUse {
    Inbox,
    Sent,
    Drafts,
    Trash,
    Junk,
    Archive,
}

/// Стабильная запись каталога; filesystem key строится только из account и ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderInfo {
    pub id: u64,
    pub name: String,
    pub special_use: Option<SpecialUse>,
}

#[derive(Debug)]
pub(super) struct FolderCatalog {
    root: PathBuf,
    accounts: HashMap<String, Vec<FolderInfo>>,
    high_ids: HashMap<String, u64>,
}

impl FolderCatalog {
    pub(super) fn load(root: &Path) -> io::Result<Self> {
        let directory = root.join("folders");
        fs::create_dir_all(&directory)?;
        let mut accounts = HashMap::new();
        let mut high_ids = HashMap::new();
        let files = super::file_key::entries(&directory, super::file_key::FileKind::FolderCatalog)?;
        for (account, path) in files {
            validate_account(&account)?;
            let replay = folder_journal::read(&path)?;
            accounts.insert(account.clone(), replay.folders);
            high_ids.insert(account, replay.high_id);
        }
        Ok(Self {
            root: directory,
            accounts,
            high_ids,
        })
    }

    /// Создаёт стандартный набор идемпотентно после crash между frames.
    pub(super) fn ensure_defaults(&mut self, account: &str) -> io::Result<()> {
        validate_account(account)?;
        for (name, special_use) in defaults() {
            let exists = self.accounts.get(account).is_some_and(|folders| {
                folders.iter().any(|folder| match special_use {
                    Some(special) => folder.special_use == Some(special),
                    None => folder.name == name,
                })
            });
            if !exists {
                self.create_with_special(account, name, special_use)?;
            }
        }
        Ok(())
    }

    pub(super) fn create(&mut self, account: &str, name: &str) -> io::Result<FolderInfo> {
        self.ensure_defaults(account)?;
        self.create_with_special(account, name, None)
    }
    pub(super) fn create_special(
        &mut self,
        account: &str,
        name: &str,
        special: SpecialUse,
    ) -> io::Result<FolderInfo> {
        self.ensure_defaults(account)?;
        self.create_with_special(account, name, Some(special))
    }

    pub(super) fn list(&mut self, account: &str) -> io::Result<Vec<FolderInfo>> {
        self.ensure_defaults(account)?;
        Ok(self
            .accounts
            .get(account)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(names::listed_name)
            .collect())
    }

    pub(super) fn resolve(&mut self, account: &str, name: &str) -> io::Result<String> {
        self.ensure_defaults(account)?;
        let folder = self.find_by_canonical_name(account, name)?;
        Ok(storage_key(account, folder))
    }

    /// Возвращает запись по key без клонирования всего каталога аккаунта.
    pub(super) fn info_by_storage_key(
        &mut self,
        account: &str,
        key: &str,
    ) -> io::Result<FolderInfo> {
        self.ensure_defaults(account)?;
        self.accounts
            .get(account)
            .into_iter()
            .flatten()
            .find(|folder| storage_key(account, folder) == key)
            .cloned()
            .map(names::listed_name)
            .ok_or_else(missing)
    }

    /// Переименовывает папку и всё её поддерево одним journal-событием.
    pub(super) fn rename(
        &mut self,
        account: &str,
        old_name: &str,
        new_name: &str,
    ) -> io::Result<()> {
        self.ensure_defaults(account)?;
        let new_name = crate::unicode::nfc::normalize(new_name);
        validate_name(&new_name)?;
        let old_name = self.find_by_canonical_name(account, old_name)?.name.clone();
        let folders = self.accounts.get(account).ok_or_else(missing)?;
        if names::has_rename_collision(folders, &old_name, &new_name) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "rename collides with a canonical folder name",
            ));
        }
        folder_journal::validate_rename(folders, &old_name, &new_name)?;
        folder_journal::append_rename(&self.root, account, &old_name, &new_name)?;
        let folders = self.accounts.get_mut(account).ok_or_else(missing)?;
        folder_journal::apply_rename(folders, &old_name, &new_name)
    }

    /// Удаляет имя папки; её устойчивый ID больше никогда не переиспользуется.
    pub(super) fn delete(&mut self, account: &str, name: &str) -> io::Result<()> {
        self.ensure_defaults(account)?;
        let name = self.find_by_canonical_name(account, name)?.name.clone();
        let folders = self.accounts.get(account).ok_or_else(missing)?;
        folder_journal::validate_delete(folders, &name)?;
        folder_journal::append_delete(&self.root, account, &name)?;
        let folders = self.accounts.get_mut(account).ok_or_else(missing)?;
        folder_journal::apply_delete(folders, &name)
    }

    fn create_with_special(
        &mut self,
        account: &str,
        name: &str,
        special_use: Option<SpecialUse>,
    ) -> io::Result<FolderInfo> {
        let name = names::canonical_name(name);
        validate_name(&name)?;
        let folders = self.accounts.entry(account.to_owned()).or_default();
        if folders
            .iter()
            .any(|folder| names::canonical_name(&folder.name) == name)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "folder exists",
            ));
        }
        let id = self
            .high_ids
            .get(account)
            .copied()
            .unwrap_or_default()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("folder id exhausted"))?;
        let folder = FolderInfo {
            id,
            name: name.clone(),
            special_use,
        };
        folder_journal::append_create(&self.root, account, &folder)?;
        folders.push(folder.clone());
        self.high_ids.insert(account.to_owned(), id);
        Ok(folder)
    }
}

fn defaults() -> [(&'static str, Option<SpecialUse>); 8] {
    [
        ("INBOX", Some(SpecialUse::Inbox)),
        ("Sent", Some(SpecialUse::Sent)),
        ("Drafts", Some(SpecialUse::Drafts)),
        ("Trash", Some(SpecialUse::Trash)),
        ("Junk", Some(SpecialUse::Junk)),
        ("Archive", Some(SpecialUse::Archive)),
        ("Invoices", None),
        ("Invoices/2026", None),
    ]
}

pub(super) fn storage_key(account: &str, folder: &FolderInfo) -> String {
    match folder.special_use {
        Some(SpecialUse::Inbox) => account.to_owned(),
        _ => format!("{account}-folder-{}", folder.id),
    }
}

pub(super) fn encode_special(value: Option<SpecialUse>) -> u8 {
    match value {
        None => 0,
        Some(SpecialUse::Inbox) => 1,
        Some(SpecialUse::Sent) => 2,
        Some(SpecialUse::Drafts) => 3,
        Some(SpecialUse::Trash) => 4,
        Some(SpecialUse::Junk) => 5,
        Some(SpecialUse::Archive) => 6,
    }
}

pub(super) fn decode_special(value: u8) -> io::Result<Option<SpecialUse>> {
    match value {
        0 => Ok(None),
        1 => Ok(Some(SpecialUse::Inbox)),
        2 => Ok(Some(SpecialUse::Sent)),
        3 => Ok(Some(SpecialUse::Drafts)),
        4 => Ok(Some(SpecialUse::Trash)),
        5 => Ok(Some(SpecialUse::Junk)),
        6 => Ok(Some(SpecialUse::Archive)),
        _ => Err(invalid("unknown special-use value")),
    }
}

pub(super) fn validate_account(value: &str) -> io::Result<()> {
    let valid = !value.is_empty()
        && value.len() <= crate::smtp::MAX_LOCAL_PART_SIZE
        && value.chars().all(|character| !character.is_control());
    match valid {
        true => Ok(()),
        false => Err(invalid("invalid account in folder catalog")),
    }
}

pub(super) fn validate_name(value: &str) -> io::Result<()> {
    match (
        value.is_empty(),
        value.len() <= MAX_NAME,
        value.chars().any(char::is_control),
    ) {
        (false, true, false) => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid folder name",
        )),
    }
}

pub(super) fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |sum, byte| {
        (sum ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn missing() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "folder does not exist")
}
