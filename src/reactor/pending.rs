//! Корреляция асинхронной store-операции с ожидающим протокольным ответом.

use crate::imap;

/// Команда определяет корректный tagged completion после общей проверки пароля.
#[derive(Clone, Copy)]
pub(super) enum AuthenticationCommand {
    Login,
    Authenticate,
}

/// Контекст остаётся у session generation до получения completion.
#[derive(Clone)]
pub(super) enum Pending {
    Authenticate {
        tag: String,
        account: String,
        command: AuthenticationCommand,
    },
    SmtpAuthenticate {
        account: String,
    },
    /// Пароль уже проверен worker-ом, далее нужен атомарный snapshot INBOX.
    Pop3Authenticate {
        account: String,
    },
    /// Snapshot и внутренний folder key возвращаются одним completion worker-а.
    Pop3ReadInbox,
    /// POP3 применяет накопленные DELE только после durable EXPUNGE.
    Pop3Commit,
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
    ReadForFetch {
        addressing: imap::Addressing,
        sequence_set: imap::sequence_set::MessageSet,
        attributes: Vec<imap::FetchAttribute>,
        tag: String,
        mailbox: String,
        read_only: bool,
        saved_uids: std::collections::BTreeSet<u64>,
    },
    /// BODY устанавливает \Seen только после durable snapshot и mutation.
    FetchReadForSeen {
        addressing: imap::Addressing,
        sequence_set: imap::sequence_set::MessageSet,
        attributes: Vec<imap::FetchAttribute>,
        tag: String,
        mailbox: String,
        uids: Vec<u64>,
        saved_uids: std::collections::BTreeSet<u64>,
    },
    ReadForStore {
        addressing: imap::Addressing,
        sequence_set: imap::sequence_set::SequenceSet,
        mutation: crate::mail_flags::FlagMutation,
        flags: crate::mail_flags::MessageFlags,
        keywords: std::collections::BTreeSet<crate::mail_flags::KeywordFlag>,
        silent: bool,
        tag: String,
        mailbox: String,
    },
    ReadForSearch {
        addressing: imap::Addressing,
        key: imap::search::SearchKey,
        charset: imap::search_charset::Charset,
        result: imap::search_result::ResultRequest,
        tag: String,
    },
    WriteFlags {
        addressing: imap::Addressing,
        uids: Vec<u64>,
        mailbox: String,
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
        special_use_only: bool,
    },
    SetSubscription {
        tag: String,
        subscribed: bool,
    },
    ListSubscriptions {
        tag: String,
        pattern: imap::folder::ListPattern,
        response: SubscriptionResponse,
    },
    Status {
        tag: String,
        name: String,
        attributes: Vec<imap::status::StatusAttribute>,
    },
    ReadForExpunge {
        tag: String,
        mailbox: String,
        close: bool,
        uid_set: Option<imap::sequence_set::SequenceSet>,
    },
    Expunge {
        tag: String,
        mailbox: String,
        sequence_numbers: Vec<usize>,
        close: bool,
    },
    ReadForCopy {
        tag: String,
        transfer: imap::Transfer,
        addressing: imap::Addressing,
        sequence_set: imap::sequence_set::SequenceSet,
        account: String,
        source: String,
        destination_name: String,
    },
    ResolveCopyDestination {
        tag: String,
        transfer: imap::Transfer,
        source: String,
        source_uids: Vec<u64>,
        source_sequences: Vec<usize>,
    },
    Copy {
        tag: String,
        transfer: imap::Transfer,
        source: String,
        destination: String,
        source_uids: Vec<u64>,
        source_sequences: Vec<usize>,
    },
    ManageSieveAuthenticate {
        account: String,
    },
    ManageSieveCommand,
}

/// Разделяет legacy LSUB и RFC LIST response, не смешивая их на wire.
#[derive(Clone)]
pub(super) enum SubscriptionResponse {
    Lsub,
    List {
        selection: imap::folder::ListSelection,
        returns: imap::folder::ListReturnOptions,
    },
}
