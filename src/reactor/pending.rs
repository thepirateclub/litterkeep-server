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
    ReadForSelect {
        tag: String,
        mailbox: String,
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
}
