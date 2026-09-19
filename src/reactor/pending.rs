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
    Append,
    ImapAppend {
        tag: String,
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
}
