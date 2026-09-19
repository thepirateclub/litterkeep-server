//! Восстановление компактного writer-state для следующего UID каждого mailbox.
//!
//! Журнал сканируется один раз при открытии Store. Последующие append используют
//! `HashMap` со средней сложностью O(1), а не повторяют O(n) replay перед каждым
//! письмом. Единственный storage writer остаётся владельцем этой карты.
use super::{journal, model};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Строит следующий UID из последнего проверенного frame каждого mailbox.
pub(super) fn load_next_uids(root: &Path) -> io::Result<HashMap<String, u64>> {
    let mut next_uids = HashMap::new();
    for entry in fs::read_dir(root.join("mailboxes"))? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("journal") {
            continue;
        }
        let mailbox = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "non-utf8 journal"))?;
        model::validate_mailbox(mailbox)?;
        let records = journal::records(mailbox, &path)?;
        let next = match records.last() {
            Some(record) => record
                .uid
                .checked_add(1)
                .ok_or_else(|| io::Error::other("uid exhausted"))?,
            None => 1,
        };
        next_uids.insert(mailbox.to_owned(), next);
    }
    Ok(next_uids)
}
