//! Append-only события каталога папок и их детерминированное восстановление.

use super::folder_cursor::{Cursor, short};
use super::folders::{self, FolderInfo, SpecialUse};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const CREATE_MAGIC: [u8; 4] = *b"LKFO";
const RENAME_MAGIC: [u8; 4] = *b"LKFR";
const DELETE_MAGIC: [u8; 4] = *b"LKFD";
const VERSION: u16 = 1;
const COMMIT: u32 = 0xf01d_c011;

pub(super) struct Replay {
    pub(super) folders: Vec<FolderInfo>,
    pub(super) high_id: u64,
}

pub(super) fn read(path: &Path) -> io::Result<Replay> {
    let bytes = fs::read(path)?;
    let mut cursor = Cursor::new(&bytes);
    let mut replay = Replay {
        folders: Vec::new(),
        high_id: 0,
    };
    while cursor.remaining() > 0 {
        let start = cursor.position();
        let result = decode_event(&mut cursor, start, &mut replay);
        match result {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                let file = OpenOptions::new().write(true).open(path)?;
                file.set_len(start as u64)?;
                file.sync_all()?;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(replay)
}

pub(super) fn append_create(root: &Path, account: &str, folder: &FolderInfo) -> io::Result<()> {
    let mut frame = header(CREATE_MAGIC);
    frame.extend_from_slice(&folder.id.to_be_bytes());
    frame.push(folders::encode_special(folder.special_use));
    push_name(&mut frame, &folder.name)?;
    append(root, account, finish(frame))
}

pub(super) fn append_rename(root: &Path, account: &str, old: &str, new: &str) -> io::Result<()> {
    let mut frame = header(RENAME_MAGIC);
    push_name(&mut frame, old)?;
    push_name(&mut frame, new)?;
    append(root, account, finish(frame))
}

pub(super) fn append_delete(root: &Path, account: &str, name: &str) -> io::Result<()> {
    let mut frame = header(DELETE_MAGIC);
    push_name(&mut frame, name)?;
    append(root, account, finish(frame))
}

pub(super) fn validate_rename(folders: &[FolderInfo], old: &str, new: &str) -> io::Result<()> {
    folders::validate_name(old)?;
    folders::validate_name(new)?;
    match (old == new, descendant(new, old).is_some()) {
        (true, _) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "destination exists",
            ));
        }
        (_, true) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination is inside source",
            ));
        }
        _ => {}
    }
    let source = folders
        .iter()
        .find(|folder| folder.name == old)
        .ok_or_else(missing)?;
    if source.special_use == Some(SpecialUse::Inbox) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "INBOX cannot be renamed",
        ));
    }
    let renamed: HashSet<String> = folders
        .iter()
        .filter_map(|folder| renamed_name(&folder.name, old, new))
        .collect();
    let collision = folders.iter().any(|folder| {
        renamed.contains(&folder.name) && renamed_name(&folder.name, old, new).is_none()
    });
    match collision {
        true => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "rename collision",
        )),
        false => Ok(()),
    }
}

pub(super) fn apply_rename(folders: &mut [FolderInfo], old: &str, new: &str) -> io::Result<()> {
    validate_rename(folders, old, new)?;
    for folder in folders {
        if let Some(name) = renamed_name(&folder.name, old, new) {
            folder.name = name;
        }
    }
    Ok(())
}

pub(super) fn validate_delete(folders: &[FolderInfo], name: &str) -> io::Result<()> {
    let folder = folders
        .iter()
        .find(|folder| folder.name == name)
        .ok_or_else(missing)?;
    if folder.special_use == Some(SpecialUse::Inbox) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "INBOX cannot be deleted",
        ));
    }
    let has_children = folders
        .iter()
        .any(|folder| descendant(&folder.name, name).is_some());
    match has_children {
        true => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "folder has children",
        )),
        false => Ok(()),
    }
}

pub(super) fn apply_delete(folders: &mut Vec<FolderInfo>, name: &str) -> io::Result<()> {
    validate_delete(folders, name)?;
    let position = folders
        .iter()
        .position(|folder| folder.name == name)
        .ok_or_else(missing)?;
    folders.remove(position);
    Ok(())
}

fn decode_event(cursor: &mut Cursor<'_>, start: usize, replay: &mut Replay) -> io::Result<()> {
    let magic = cursor.take::<4>()?;
    if u16::from_be_bytes(cursor.take::<2>()?) != VERSION {
        return Err(folders::invalid("unsupported folder frame"));
    }
    match magic {
        CREATE_MAGIC => {
            let folder = decode_create(cursor)?;
            verify(cursor, start)?;
            apply_create(replay, folder)
        }
        RENAME_MAGIC => {
            let old = take_name(cursor)?;
            let new = take_name(cursor)?;
            verify(cursor, start)?;
            apply_rename(&mut replay.folders, &old, &new)
        }
        DELETE_MAGIC => {
            let name = take_name(cursor)?;
            verify(cursor, start)?;
            apply_delete(&mut replay.folders, &name)
        }
        _ => Err(folders::invalid("unknown folder event")),
    }
}

fn decode_create(cursor: &mut Cursor<'_>) -> io::Result<FolderInfo> {
    let id = u64::from_be_bytes(cursor.take::<8>()?);
    let special_use = folders::decode_special(u8::from_be_bytes(cursor.take::<1>()?))?;
    let name = take_name(cursor)?;
    Ok(FolderInfo {
        id,
        name,
        special_use,
    })
}

fn apply_create(replay: &mut Replay, folder: FolderInfo) -> io::Result<()> {
    let id = folder.id;
    let invalid_order = id <= replay.high_id;
    let duplicate = replay
        .folders
        .iter()
        .any(|existing| existing.name == folder.name);
    match (invalid_order, duplicate) {
        (false, false) => {
            replay.high_id = id;
            replay.folders.push(folder);
            Ok(())
        }
        _ => Err(folders::invalid("duplicate or unordered folder")),
    }
}

fn append(root: &Path, account: &str, frame: Vec<u8>) -> io::Result<()> {
    let path = super::file_key::prepare(root, account, super::file_key::FileKind::FolderCatalog)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(&frame)?;
    file.sync_all()?;
    let directory = path.parent().unwrap_or(root);
    super::file_key::sync_key_directory(root, directory)
}

fn header(magic: [u8; 4]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(64);
    frame.extend_from_slice(&magic);
    frame.extend_from_slice(&VERSION.to_be_bytes());
    frame
}

fn push_name(frame: &mut Vec<u8>, name: &str) -> io::Result<()> {
    folders::validate_name(name)?;
    let length = u16::try_from(name.len()).map_err(|_| folders::invalid("folder name limit"))?;
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(name.as_bytes());
    Ok(())
}

fn take_name(cursor: &mut Cursor<'_>) -> io::Result<String> {
    let length = usize::from(u16::from_be_bytes(cursor.take::<2>()?));
    if length > folders::MAX_NAME {
        return Err(folders::invalid("folder name limit"));
    }
    let name = std::str::from_utf8(cursor.take_slice(length)?)
        .map_err(|_| folders::invalid("folder name is not utf8"))?
        .to_owned();
    folders::validate_name(&name)?;
    Ok(name)
}

fn finish(mut frame: Vec<u8>) -> Vec<u8> {
    frame.extend_from_slice(&folders::checksum(&frame).to_be_bytes());
    frame.extend_from_slice(&COMMIT.to_be_bytes());
    frame
}

fn verify(cursor: &mut Cursor<'_>, start: usize) -> io::Result<()> {
    let checksum_position = cursor.position();
    let expected = u64::from_be_bytes(cursor.take::<8>()?);
    let commit = u32::from_be_bytes(cursor.take::<4>()?);
    let protected = cursor
        .bytes
        .get(start..checksum_position)
        .ok_or_else(short)?;
    match (expected == folders::checksum(protected), commit == COMMIT) {
        (true, true) => Ok(()),
        _ => Err(folders::invalid("invalid folder frame trailer")),
    }
}

pub(super) fn renamed_name(name: &str, old: &str, new: &str) -> Option<String> {
    match name == old {
        true => Some(new.to_owned()),
        false => descendant(name, old).map(|suffix| format!("{new}/{suffix}")),
    }
}

fn descendant<'a>(name: &'a str, parent: &str) -> Option<&'a str> {
    name.strip_prefix(parent)?.strip_prefix('/')
}

fn missing() -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, "folder does not exist")
}
