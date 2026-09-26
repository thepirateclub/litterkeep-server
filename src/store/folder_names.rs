//! Разрешение NFC-псевдонимов и проекция долговечных имён папок.

use super::{FolderCatalog, FolderInfo, SpecialUse};
use std::collections::HashSet;
use std::io;

impl FolderCatalog {
    /// Находит канонический псевдоним, не переписывая долговечное имя.
    pub(super) fn find_by_canonical_name(
        &self,
        account: &str,
        name: &str,
    ) -> io::Result<&FolderInfo> {
        let wanted = canonical_name(name);
        let candidates = self
            .accounts
            .get(account)
            .into_iter()
            .flatten()
            .filter(|folder| match folder.special_use {
                Some(SpecialUse::Inbox) => name.eq_ignore_ascii_case("INBOX"),
                _ => canonical_name(&folder.name) == wanted,
            });
        let mut candidates = candidates;
        match (candidates.next(), candidates.next()) {
            (Some(folder), None) => Ok(folder),
            (None, _) => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "folder does not exist",
            )),
            (Some(_), Some(_)) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ambiguous canonical folder name",
            )),
        }
    }
}

/// Возвращает NFC-имя папки, сохраняя её стабильный внутренний идентификатор.
fn canonical_folder_name(mut folder: FolderInfo) -> FolderInfo {
    folder.name = canonical_name(&folder.name);
    folder
}

pub(super) fn canonical_name(name: &str) -> String {
    crate::unicode::nfc::normalize(name)
}

pub(super) fn listed_name(folder: FolderInfo) -> FolderInfo {
    canonical_folder_name(folder)
}

/// Отклоняет переименование, создающее коллизию канонических имён.
pub(super) fn has_rename_collision(folders: &[FolderInfo], old: &str, new: &str) -> bool {
    let renamed_names = folders
        .iter()
        .filter_map(|folder| super::folder_journal::renamed_name(&folder.name, old, new))
        .map(|name| canonical_name(&name))
        .collect::<HashSet<_>>();
    folders.iter().any(|folder| {
        super::folder_journal::renamed_name(&folder.name, old, new).is_none()
            && renamed_names.contains(&canonical_name(&folder.name))
    })
}
