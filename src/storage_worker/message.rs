//! Типизированные сообщения bounded-канала между reactor и владельцем store.

use crate::mail_flags::{FlagMutation, MessageFlags};
use crate::store::{MailboxSnapshot, ResolvedMailbox, SubscriptionSnapshot};
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
        /// SMTP MAIL FROM до добавления trace-заголовков. Это не `From` MIME.
        envelope_sender: String,
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
        keywords: std::collections::BTreeSet<crate::mail_flags::KeywordFlag>,
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
    SelectFolder {
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
        include_status: bool,
        include_size: bool,
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
    SievePut {
        request_id: u64,
        account: String,
        name: String,
        source: String,
    },
    SieveList {
        request_id: u64,
        account: String,
    },
    SieveSetActive {
        request_id: u64,
        account: String,
        name: Option<String>,
    },
    SieveGet {
        request_id: u64,
        account: String,
        name: String,
    },
    SieveDelete {
        request_id: u64,
        account: String,
        name: String,
    },
    SieveRename {
        request_id: u64,
        account: String,
        old: String,
        new: String,
    },
    SieveHaveSpace {
        request_id: u64,
        account: String,
        name: String,
        size: u64,
    },
    SieveCheck {
        request_id: u64,
        source: String,
    },
    /// Публикует DSN только после account/local-route проверки владельцем Store.
    AppendDsn {
        request_id: u64,
        reverse_path: String,
        recipient: String,
        transaction_id: u64,
        original: Vec<u8>,
        failure: crate::dsn::Failure,
    },
    OutboundPending {
        request_id: u64,
    },
    /// Получает лишь ограниченную пачку due jobs у владельца Store.
    OutboundDue {
        request_id: u64,
        now_epoch: u64,
        limit: usize,
    },
    OutboundBlob {
        request_id: u64,
        delivery: crate::queue::OwnedDelivery,
    },
    OutboundTransition {
        request_id: u64,
        delivery: crate::queue::OwnedDelivery,
        state: crate::queue::State,
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
    FolderSelected {
        request_id: u64,
        result: io::Result<ResolvedMailbox>,
    },
    SubscriptionSet {
        request_id: u64,
        result: io::Result<()>,
    },
    Subscriptions {
        request_id: u64,
        result: io::Result<SubscriptionSnapshot>,
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
    SieveStored {
        request_id: u64,
        result: io::Result<crate::store::ScriptInfo>,
    },
    SieveScripts {
        request_id: u64,
        result: io::Result<Vec<crate::store::ScriptInfo>>,
    },
    SieveSource {
        request_id: u64,
        result: io::Result<(u64, String)>,
    },
    SieveMutated {
        request_id: u64,
        result: io::Result<()>,
    },
    SieveSpace {
        request_id: u64,
        result: io::Result<bool>,
    },
    /// `None` означает пустой или внешний reverse-path без локальной публикации.
    DsnAppended {
        request_id: u64,
        result: io::Result<Option<u64>>,
    },
    OutboundPending {
        request_id: u64,
        result: io::Result<Vec<crate::queue::OwnedDelivery>>,
    },
    OutboundDue {
        request_id: u64,
        result: io::Result<Vec<crate::queue::OwnedDelivery>>,
    },
    OutboundBlob {
        request_id: u64,
        result: io::Result<Vec<u8>>,
    },
    OutboundTransitioned {
        request_id: u64,
        result: io::Result<()>,
    },
}
