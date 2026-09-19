use super::{durability, model::blob_name};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
pub fn next_available(root: &Path, transaction_id: &mut u64) -> io::Result<(String, PathBuf)> {
    loop {
        let name = blob_name(*transaction_id);
        let path = root.join("blobs").join(&name);
        if !path.exists() {
            return Ok((name, path));
        }
        *transaction_id = transaction_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("transaction id exhausted"))?
    }
}
pub fn publish(root: &Path, name: &str, raw: &[u8]) -> io::Result<()> {
    let final_path = root.join("blobs").join(name);
    let temp = root.join("blobs").join(format!(".{name}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(raw)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, final_path)?;
    durability::sync_directory(&root.join("blobs"))
}
