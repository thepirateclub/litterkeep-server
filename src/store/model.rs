use std::{io, path::PathBuf};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub uid: u64,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Record {
    pub uid: u64,
    pub transaction_id: u64,
    pub name: String,
}
pub fn validate_mailbox(mailbox: &str) -> io::Result<()> {
    let characters_are_safe = mailbox
        .bytes()
        .all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'));
    match (mailbox.is_empty(), characters_are_safe) {
        (true, _) | (false, false) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe mailbox",
        )),
        (false, true) => Ok(()),
    }
}
pub fn blob_name(transaction_id: u64) -> String {
    format!("message-{transaction_id}.eml")
}
