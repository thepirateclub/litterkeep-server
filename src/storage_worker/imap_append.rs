//! Единый Store-owner переход от APPEND mailbox name к durable batch.

use crate::store::{AppendMetadata, Store};
use std::io;

/// Durable результат пакетной вставки с каноническим mailbox key.
#[derive(Debug)]
pub struct ImapAppendResult {
    pub mailbox: String,
    pub uids: Vec<u64>,
}

/// Разрешает account-scoped mailbox и коммитит все APPEND messages атомарно.
pub(super) fn execute(
    store: &mut Store,
    account: &str,
    mailbox: &str,
    messages: &[(Vec<u8>, AppendMetadata)],
) -> io::Result<ImapAppendResult> {
    let resolved = store.resolve_folder(account, mailbox)?;
    let uids = store.append_batch(&resolved, messages)?;
    Ok(ImapAppendResult {
        mailbox: resolved,
        uids,
    })
}
