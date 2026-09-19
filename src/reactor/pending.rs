//! Корреляция асинхронной store-операции с ожидающим протокольным ответом.

use crate::imap;

/// Контекст остаётся у session generation до получения completion.
#[derive(Clone)]
pub(super) enum Pending {
    Authenticate {
        tag: String,
        account: String,
    },
    SmtpAuthenticate {
        account: String,
    },
    ValidateRecipient {
        address: String,
    },
    Append {
        mailboxes: Vec<String>,
    },
    ImapAppend {
        tag: String,
        mailbox: String,
    },
    ResolveForAppend {
        tag: String,
        literal_size: usize,
        metadata: crate::store::AppendMetadata,
    },
    ReadForSelect {
        tag: String,
        read_only: bool,
    },
    ReadForFetch(
        imap::Addressing,
        imap::sequence_set::SequenceSet,
        imap::FetchAttribute,
        String,
    ),
    ReadForStore {
        addressing: imap::Addressing,
        sequence_set: imap::sequence_set::SequenceSet,
        mutation: crate::mail_flags::FlagMutation,
        flags: crate::mail_flags::MessageFlags,
        silent: bool,
        tag: String,
        mailbox: String,
    },
    WriteFlags {
        addressing: imap::Addressing,
        uids: Vec<u64>,
        silent: bool,
        tag: String,
    },
    CreateFolder {
        tag: String,
    },
    MutateFolder {
        tag: String,
        operation: imap::folder::Mutation,
    },
    ListFolders {
        tag: String,
        pattern: imap::folder::ListPattern,
    },
    SetSubscription {
        tag: String,
        subscribed: bool,
    },
    ListSubscriptions {
        tag: String,
        pattern: imap::folder::ListPattern,
    },
    Status {
        tag: String,
        name: String,
        attributes: Vec<imap::status::StatusAttribute>,
    },
}
