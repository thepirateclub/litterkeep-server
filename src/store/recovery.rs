use super::{commit_log, journal};
use std::fs;
use std::io;
use std::path::Path;
pub fn recover(root: &Path) -> io::Result<()> {
    clean_temps(&root.join("blobs"))?;
    commit_log::recover(&root.join("journal/transactions.log"))?;
    super::smtp_dsn_journal::recover(root)?;
    let mailbox_files =
        super::file_key::entries(&root.join("mailboxes"), super::file_key::FileKind::Journal)?;
    for (mailbox, path) in mailbox_files {
        journal::recover(&mailbox, &path)?;
        journal::records(&mailbox, &path)?;
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
