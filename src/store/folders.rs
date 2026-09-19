//! Долговечный каталог папок и отображение пользовательских имён на внутренние ID.

use super::durability;
use super::folder_cursor::{Cursor, short};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"LKFO";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xf01d_c011;
const MAX_NAME: usize = 120;

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
}

impl FolderCatalog {
    pub(super) fn load(root: &Path) -> io::Result<Self> {
        let directory = root.join("folders");
        fs::create_dir_all(&directory)?;
        let mut accounts = HashMap::new();
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("journal") {
                continue;
            }
            let account = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| invalid("non-utf8 folder journal"))?;
            validate_account(account)?;
            accounts.insert(account.to_owned(), read_records(&path)?);
        }
        Ok(Self {
            root: directory,
            accounts,
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

    pub(super) fn list(&mut self, account: &str) -> io::Result<Vec<FolderInfo>> {
        self.ensure_defaults(account)?;
        Ok(self.accounts.get(account).cloned().unwrap_or_default())
    }

    pub(super) fn resolve(&mut self, account: &str, name: &str) -> io::Result<String> {
        self.ensure_defaults(account)?;
        let folder = self
            .accounts
            .get(account)
            .into_iter()
            .flatten()
            .find(|folder| match folder.special_use {
                Some(SpecialUse::Inbox) => name.eq_ignore_ascii_case("INBOX"),
                _ => folder.name == name,
            })
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "folder does not exist"))?;
        Ok(storage_key(account, folder))
    }

    fn create_with_special(
        &mut self,
        account: &str,
        name: &str,
        special_use: Option<SpecialUse>,
    ) -> io::Result<FolderInfo> {
        validate_name(name)?;
        let folders = self.accounts.entry(account.to_owned()).or_default();
        if folders.iter().any(|folder| folder.name == name) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "folder exists",
            ));
        }
        let id = folders
            .last()
            .map(|folder| folder.id)
            .unwrap_or_default()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("folder id exhausted"))?;
        let folder = FolderInfo {
            id,
            name: name.to_owned(),
            special_use,
        };
        let path = self.root.join(format!("{account}.journal"));
        let durable = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| {
                file.write_all(&encode(&folder)?)
                    .and_then(|()| file.sync_all())
            })
            .and_then(|()| durability::sync_directory(&self.root));
        durable?;
        folders.push(folder.clone());
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

fn storage_key(account: &str, folder: &FolderInfo) -> String {
    match folder.special_use {
        Some(SpecialUse::Inbox) => account.to_owned(),
        _ => format!("{account}-folder-{}", folder.id),
    }
}

fn encode(folder: &FolderInfo) -> io::Result<Vec<u8>> {
    let name = folder.name.as_bytes();
    let length = u16::try_from(name.len()).map_err(|_| invalid("folder name limit"))?;
    let mut bytes = Vec::with_capacity(29 + name.len());
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_be_bytes());
    bytes.extend_from_slice(&folder.id.to_be_bytes());
    bytes.push(encode_special(folder.special_use));
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(name);
    let checksum = checksum(&bytes);
    bytes.extend_from_slice(&checksum.to_be_bytes());
    bytes.extend_from_slice(&COMMIT.to_be_bytes());
    Ok(bytes)
}

fn read_records(path: &Path) -> io::Result<Vec<FolderInfo>> {
    let bytes = fs::read(path)?;
    let mut cursor = Cursor::new(&bytes);
    let mut folders = Vec::new();
    while cursor.remaining() > 0 {
        let start = cursor.position();
        let folder = match decode(&mut cursor, start) {
            Ok(folder) => folder,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                let file = OpenOptions::new().write(true).open(path)?;
                file.set_len(start as u64)?;
                file.sync_all()?;
                break;
            }
            Err(error) => return Err(error),
        };
        if folders
            .iter()
            .any(|existing: &FolderInfo| existing.id >= folder.id || existing.name == folder.name)
        {
            return Err(invalid("duplicate or unordered folder"));
        }
        folders.push(folder);
    }
    Ok(folders)
}

fn decode(cursor: &mut Cursor<'_>, start: usize) -> io::Result<FolderInfo> {
    if cursor.take::<4>()? != MAGIC || u16::from_be_bytes(cursor.take::<2>()?) != VERSION {
        return Err(invalid("unsupported folder frame"));
    }
    let id = u64::from_be_bytes(cursor.take::<8>()?);
    let special_use = decode_special(u8::from_be_bytes(cursor.take::<1>()?))?;
    let length = usize::from(u16::from_be_bytes(cursor.take::<2>()?));
    if length > MAX_NAME {
        return Err(invalid("folder name limit"));
    }
    let name = std::str::from_utf8(cursor.take_slice(length)?)
        .map_err(|_| invalid("folder name is not utf8"))?
        .to_owned();
    validate_name(&name)?;
    let checksum_position = cursor.position();
    let expected = u64::from_be_bytes(cursor.take::<8>()?);
    let commit = u32::from_be_bytes(cursor.take::<4>()?);
    let protected = cursor
        .bytes
        .get(start..checksum_position)
        .ok_or_else(short)?;
    if (expected, commit) != (checksum(protected), COMMIT) {
        return Err(invalid("invalid folder frame trailer"));
    }
    Ok(FolderInfo {
        id,
        name,
        special_use,
    })
}

fn encode_special(value: Option<SpecialUse>) -> u8 {
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

fn decode_special(value: u8) -> io::Result<Option<SpecialUse>> {
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

fn validate_account(value: &str) -> io::Result<()> {
    match value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        true if !value.is_empty() => Ok(()),
        _ => Err(invalid("unsafe account in folder catalog")),
    }
}

fn validate_name(value: &str) -> io::Result<()> {
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

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |sum, byte| {
        (sum ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
