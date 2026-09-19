use super::{commit_log, journal, model};
use std::fs;
use std::io;
use std::path::Path;
pub fn recover(root: &Path) -> io::Result<()> {
    clean_temps(&root.join("blobs"))?;
    commit_log::recover(&root.join("journal/transactions.log"))?;
    for entry in fs::read_dir(root.join("mailboxes"))? {
        let path = entry?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("journal") {
            continue;
        }
        let mailbox = path
            .file_stem()
            .and_then(|v| v.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "non-utf8 journal"))?;
        model::validate_mailbox(mailbox)?;
        journal::recover(mailbox, &path)?;
        journal::records(mailbox, &path)?;
    }
    Ok(())
}
fn clean_temps(dir: &Path) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let is_temporary = path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(
                |value| match (value.starts_with('.'), value.ends_with(".tmp")) {
                    (true, true) => true,
                    (true, false) | (false, _) => false,
                },
            );
        if is_temporary {
            fs::remove_file(path)?
        }
    }
    Ok(())
}
