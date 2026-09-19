//! Типизированные сообщения bounded-канала между reactor и владельцем store.

use crate::mail_flags::{FlagMutation, MessageFlags};
use crate::store::{MailboxSnapshot, ResolvedMailbox, SubscribedFolder};
use std::io;

#[derive(Debug)]
pub enum Request {
    Authenticate {
        request_id: u64,
        username: String,
        password: Vec<u8>,
    },
    ValidateRecipient {
        request_id: u64,
        mailbox: String,
    },
    Append {
        request_id: u64,
        mailbox: String,
        bytes: Vec<u8>,
        metadata: crate::store::AppendMetadata,
    },
    AppendMany {
        request_id: u64,
        mailboxes: Vec<String>,
        bytes: Vec<u8>,
    },
    Read {
        request_id: u64,
        mailbox: String,
    },
    UpdateFlags {
        request_id: u64,
        mailbox: String,
        uids: Vec<u64>,
        mutation: FlagMutation,
        flags: MessageFlags,
    },
    CreateFolder {
        request_id: u64,
        account: String,
        name: String,
        special_use: Option<crate::store::SpecialUse>,
    },
    RenameFolder {
        request_id: u64,
        account: String,
        old_name: String,
        new_name: String,
    },
    DeleteFolder {
        request_id: u64,
        account: String,
        name: String,
    },
    ListFolders {
        request_id: u64,
        account: String,
    },
    ResolveFolder {
        request_id: u64,
        account: String,
        name: String,
    },
    ReadFolder {
        request_id: u64,
        account: String,
        name: String,
    },
    SetSubscription {
        request_id: u64,
        account: String,
        name: String,
        subscribed: bool,
    },
    ListSubscriptions {
        request_id: u64,
        account: String,
    },
    Expunge {
        request_id: u64,
        mailbox: String,
        uids: Vec<u64>,
    },
    Copy {
        request_id: u64,
        source: String,
        destination: String,
        uids: Vec<u64>,
    },
    Move {
        request_id: u64,
        source: String,
        destination: String,
        uids: Vec<u64>,
    },
}

#[derive(Debug)]
pub enum Response {
    Authenticated {
        request_id: u64,
        accepted: bool,
    },
    RecipientValidated {
        request_id: u64,
        accepted: bool,
    },
    Appended {
        request_id: u64,
        result: io::Result<u64>,
    },
    Read {
        request_id: u64,
        result: io::Result<MailboxSnapshot>,
    },
    FlagsUpdated {
        request_id: u64,
        result: io::Result<MailboxSnapshot>,
    },
    FolderCreated {
        request_id: u64,
        result: io::Result<crate::store::FolderInfo>,
    },
    FolderMutated {
        request_id: u64,
        result: io::Result<()>,
    },
    Folders {
        request_id: u64,
        result: io::Result<Vec<crate::store::FolderInfo>>,
    },
    FolderResolved {
        request_id: u64,
        result: io::Result<String>,
    },
    FolderRead {
        request_id: u64,
        result: io::Result<ResolvedMailbox>,
    },
    SubscriptionSet {
        request_id: u64,
        result: io::Result<()>,
    },
    Subscriptions {
        request_id: u64,
        result: io::Result<Vec<SubscribedFolder>>,
    },
    Expunged {
        request_id: u64,
        result: io::Result<Vec<u64>>,
    },
    Copied {
        request_id: u64,
        result: io::Result<Vec<u64>>,
    },
    Moved {
        request_id: u64,
        result: io::Result<Vec<u64>>,
    },
}
