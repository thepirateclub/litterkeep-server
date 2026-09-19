//! Долговечный account-scoped набор подписок IMAP.

use super::durability;
use super::folder_cursor::{Cursor, short};
use super::folders::{self, SpecialUse};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"LKSB";
const VERSION: u16 = 1;
const COMMIT: u32 = 0x5ab5_c011;

/// Проекция подписки для LSUB не скрывает уже удалённое имя.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscribedFolder {
    pub name: String,
    pub exists: bool,
    pub special_use: Option<SpecialUse>,
}

#[derive(Debug)]
pub(super) struct SubscriptionCatalog {
    root: PathBuf,
    accounts: HashMap<String, BTreeSet<String>>,
}

impl SubscriptionCatalog {
    pub(super) fn load(store_root: &Path) -> io::Result<Self> {
        let root = store_root.join("subscriptions");
        fs::create_dir_all(&root)?;
        let mut accounts = HashMap::new();
        for entry in fs::read_dir(&root)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("journal") {
                continue;
            }
            let account = path
                .file_stem()
                .and_then(|value| value.to_str())
                .ok_or_else(|| folders::invalid("non-utf8 subscription journal"))?;
            folders::validate_account(account)?;
            accounts.insert(account.to_owned(), read(&path)?);
        }
        Ok(Self { root, accounts })
    }

    pub(super) fn set(&mut self, account: &str, name: &str, subscribed: bool) -> io::Result<()> {
        folders::validate_account(account)?;
        folders::validate_name(name)?;
        let names = self.accounts.entry(account.to_owned()).or_default();
        if names.contains(name) == subscribed {
            return Ok(());
        }
        append(&self.root, account, name, subscribed)?;
        match subscribed {
            true => {
                names.insert(name.to_owned());
            }
            false => {
                names.remove(name);
            }
        }
        Ok(())
    }

    pub(super) fn list(&self, account: &str) -> io::Result<Vec<String>> {
        folders::validate_account(account)?;
        Ok(self
            .accounts
            .get(account)
            .into_iter()
            .flatten()
            .cloned()
            .collect())
    }
}

fn read(path: &Path) -> io::Result<BTreeSet<String>> {
    let bytes = fs::read(path)?;
    let mut cursor = Cursor::new(&bytes);
    let mut names = BTreeSet::new();
    while cursor.remaining() > 0 {
        let start = cursor.position();
        match decode(&mut cursor, start) {
            Ok((name, true)) => {
                names.insert(name);
            }
            Ok((name, false)) => {
                names.remove(&name);
            }
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                let file = OpenOptions::new().write(true).open(path)?;
                file.set_len(start as u64)?;
                file.sync_all()?;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(names)
}

fn append(root: &Path, account: &str, name: &str, subscribed: bool) -> io::Result<()> {
    let bytes = name.as_bytes();
    let length =
        u16::try_from(bytes.len()).map_err(|_| folders::invalid("subscription name limit"))?;
    let mut frame = Vec::with_capacity(21 + bytes.len());
    frame.extend_from_slice(&MAGIC);
    frame.extend_from_slice(&VERSION.to_be_bytes());
    frame.push(u8::from(subscribed));
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(bytes);
    frame.extend_from_slice(&folders::checksum(&frame).to_be_bytes());
    frame.extend_from_slice(&COMMIT.to_be_bytes());
    let path = root.join(format!("{account}.journal"));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&frame)?;
    file.sync_all()?;
    durability::sync_directory(root)
}

fn decode(cursor: &mut Cursor<'_>, start: usize) -> io::Result<(String, bool)> {
    let magic = cursor.take::<4>()?;
    let version = u16::from_be_bytes(cursor.take::<2>()?);
    let subscribed = match u8::from_be_bytes(cursor.take::<1>()?) {
        0 => false,
        1 => true,
        _ => return Err(folders::invalid("unknown subscription operation")),
    };
    if (magic, version) != (MAGIC, VERSION) {
        return Err(folders::invalid("unsupported subscription frame"));
    }
    let length = usize::from(u16::from_be_bytes(cursor.take::<2>()?));
    if length > folders::MAX_NAME {
        return Err(folders::invalid("subscription name limit"));
    }
    let name = std::str::from_utf8(cursor.take_slice(length)?)
        .map_err(|_| folders::invalid("subscription name is not utf8"))?
        .to_owned();
    folders::validate_name(&name)?;
    let checksum_position = cursor.position();
    let expected = u64::from_be_bytes(cursor.take::<8>()?);
    let commit = u32::from_be_bytes(cursor.take::<4>()?);
    let protected = cursor
        .bytes
        .get(start..checksum_position)
        .ok_or_else(short)?;
    match (expected == folders::checksum(protected), commit == COMMIT) {
        (true, true) => Ok((name, subscribed)),
        _ => Err(folders::invalid("invalid subscription frame trailer")),
    }
}
