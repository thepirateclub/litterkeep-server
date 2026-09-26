//! Декодирование IMAP-команд: текст сети преобразуется в типизированные варианты.

pub mod append;
#[cfg(test)]
#[path = "imap/append_tests.rs"]
mod append_tests;
pub mod authenticate;
#[cfg(test)]
#[path = "imap/command_structure_tests.rs"]
mod command_structure_tests;
mod command_verb;
pub mod copy;
#[cfg(test)]
#[path = "imap/copy_tests.rs"]
mod copy_tests;
pub mod enable;
pub mod fetch;
mod fetch_section;
#[cfg(test)]
#[path = "imap/fetch_tests.rs"]
mod fetch_tests;
#[cfg(test)]
#[path = "imap/flag_grammar_tests.rs"]
mod flag_grammar_tests;
pub mod flags;
pub mod folder;
pub mod idle;
#[cfg(any(target_os = "linux", test))]
#[path = "imap/invalid_utf8_tag.rs"]
pub(crate) mod invalid_utf8_tag;
pub mod list;
#[cfg(test)]
#[path = "imap/list_mailbox_tests.rs"]
mod list_mailbox_tests;
#[path = "imap/list_reference.rs"]
mod list_reference;
#[cfg(test)]
#[path = "imap/list_tests.rs"]
mod list_tests;
pub mod mailbox_utf8;
pub mod modified_utf7;
mod parser;
pub mod search;
pub mod search_charset;
#[path = "imap/search_date.rs"]
pub mod search_date;
#[cfg(test)]
#[path = "imap/search_parser_tests.rs"]
mod search_parser_tests;
pub mod search_result;
#[path = "imap/sent_date.rs"]
pub mod sent_date;
pub mod sequence_set;
pub mod status;
#[cfg(test)]
#[path = "imap/store_tests.rs"]
mod store_tests;
mod token_values;
pub mod tokenizer;
mod unicode_17;
pub use authenticate::{AuthenticationMechanism, InitialResponse};
pub use fetch::{BodySection, FetchAttribute, Partial};
pub use idle::{IdleContinuation, parse_idle_continuation};
#[cfg(test)]
use sequence_set::SequenceSet;
/// Команда вместе с тегом, которым сервер обязан завершить ответ.
#[derive(Debug, Eq, PartialEq)]
pub struct TaggedCommand {
    pub tag: String,
    pub command: Command,
}
/// Поддерживаемый на текущем этапе набор IMAP-команд.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Capability,
    Namespace,
    Enable {
        capabilities: Vec<enable::Capability>,
    },
    StartTls,
    Login {
        username: String,
        password: Vec<u8>,
    },
    Authenticate {
        mechanism: AuthenticationMechanism,
        initial_response: InitialResponse,
    },
    Append {
        mailbox: MailboxName,
        literal_size: usize,
        flags: crate::mail_flags::MessageFlags,
        keywords: std::collections::BTreeSet<crate::mail_flags::KeywordFlag>,
        internal_date: Option<crate::mail_date::MailDate>,
    },
    Logout,
    Noop,
    Idle,
    Create {
        mailbox: MailboxName,
        special_use: Option<crate::store::SpecialUse>,
    },
    Delete {
        mailbox: MailboxName,
    },
    Rename {
        source: MailboxName,
        destination: MailboxName,
    },
    Subscribe {
        mailbox: MailboxName,
    },
    Unsubscribe {
        mailbox: MailboxName,
    },
    Lsub {
        pattern: folder::ListPattern,
    },
    Status {
        mailbox: MailboxName,
        attributes: Vec<status::StatusAttribute>,
    },
    Check,
    Close,
    Unselect,
    Copy {
        transfer: Transfer,
        addressing: Addressing,
        sequence_set: sequence_set::MessageSet,
        destination: MailboxName,
    },
    Expunge,
    UidExpunge {
        sequence_set: sequence_set::MessageSet,
    },
    List {
        pattern: folder::ListPattern,
        returns: folder::ListReturnOptions,
    },
    /// Критерии отбора extended LIST после сетевого разбора.
    ListSelection {
        pattern: folder::ListPattern,
        selection: folder::ListSelection,
        returns: folder::ListReturnOptions,
    },
    Select {
        mailbox: MailboxName,
    },
    Examine {
        mailbox: MailboxName,
    },
    Fetch {
        addressing: Addressing,
        sequence_set: sequence_set::MessageSet,
        attributes: Vec<FetchAttribute>,
    },
    Store {
        addressing: Addressing,
        sequence_set: sequence_set::MessageSet,
        mutation: crate::mail_flags::FlagMutation,
        silent: bool,
        flags: crate::mail_flags::MessageFlags,
        keywords: std::collections::BTreeSet<crate::mail_flags::KeywordFlag>,
    },
    Search {
        addressing: Addressing,
        key: search::SearchKey,
        charset: search_charset::Charset,
        charset_explicit: bool,
        result: search_result::ResultRequest,
    },
    Unsupported,
}
/// Типизированное имя mailbox после обработки специального имени INBOX.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxName {
    Inbox,
    Named(String),
}
/// Пространство номеров, в котором интерпретируется sequence-set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Addressing {
    Sequence,
    Uid,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transfer {
    Copy,
    Move,
}
/// Разбирает одну CRLF-строку и не оставляет строковые команды в reactor.
pub fn parse(line: &str) -> TaggedCommand {
    parser::parse(line)
}

/// Parser entry point для проверки команд в явно согласованном string mode.
pub fn parse_with_mode(line: &str, mode: tokenizer::StringMode) -> TaggedCommand {
    parser::parse_with_mode(line, mode)
}

/// Разбирает внутренне собранную команду в выбранном session string mode.
#[cfg(target_os = "linux")]
pub(crate) fn parse_literals_with_mode(
    line: &str,
    literals: Vec<Vec<u8>>,
    mode: tokenizer::StringMode,
) -> TaggedCommand {
    parser::parse_literals_with_mode(line, literals, mode)
}

#[cfg(test)]
#[path = "imap/mailbox_mode_tests.rs"]
mod mailbox_mode_tests;
#[cfg(test)]
#[path = "imap/simple_command_tests.rs"]
mod simple_command_tests;
#[cfg(test)]
#[path = "imap/tests.rs"]
mod tests;
