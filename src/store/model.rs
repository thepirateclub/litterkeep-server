use std::{io, path::PathBuf};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub uid: u64,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}
pub struct Record {
    pub uid: u64,
    pub name: String,
}
pub fn validate_mailbox(mailbox: &str) -> io::Result<()> {
    if mailbox.is_empty()
        || !mailbox
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe mailbox",
        ))
    } else {
        Ok(())
    }
}
pub fn blob_name(mailbox: &str, uid: u64) -> String {
    format!("{mailbox}-{uid}.eml")
}
