//! Декодирование IMAP-команд: текст сети преобразуется в типизированные варианты.

pub mod append;
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
pub mod flags;
pub mod folder;
pub mod idle;
#[cfg(any(target_os = "linux", test))]
#[path = "imap/invalid_utf8_tag.rs"]
pub(crate) mod invalid_utf8_tag;
pub mod list;
#[path = "imap/list_reference.rs"]
mod list_reference;
pub mod modified_utf7;
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
pub mod tokenizer;
pub use authenticate::{AuthenticationMechanism, InitialResponse};
use command_verb::{Verb, parse_verb};
pub use fetch::{BodySection, FetchAttribute, Partial};
pub use idle::{IdleContinuation, parse_idle_continuation};
#[cfg(test)]
use sequence_set::SequenceSet;
use tokenizer::Token;
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
        password: String,
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
    parse_tokens(line, tokenizer::tokenize(line))
}

/// Разбирает команду после внутренней сборки synchronizing SEARCH literal.
#[cfg(target_os = "linux")]
pub(crate) fn parse_search_literals(line: &str, literals: Vec<String>) -> TaggedCommand {
    parse_tokens(line, tokenizer::tokenize_search_literals(line, literals))
}

/// Преобразует lexer result в команду, сохраняя tag даже при синтаксическом отказе.
fn parse_tokens(line: &str, result: Result<Vec<Token>, &'static str>) -> TaggedCommand {
    let mut tokens = match result {
        Ok(tokens) => tokens.into_iter(),
        Err(_) => {
            let tag = line
                .split_ascii_whitespace()
                .next()
                .filter(|candidate| tokenizer::valid_tag(candidate))
                .unwrap_or("*");
            return unsupported(tag);
        }
    };
    let tag = match tokens.next() {
        Some(Token::Text(tag)) if tokenizer::valid_tag(&tag) => tag,
        _ => return unsupported("*"),
    };
    let command = match next_atom(&mut tokens).as_deref().map(parse_verb) {
        Some(Verb::Capability) => parse_empty(tokens, Command::Capability),
        Some(Verb::Namespace) => parse_empty(tokens, Command::Namespace),
        Some(Verb::Enable) => enable::parse(tokens),
        Some(Verb::StartTls) => parse_empty(tokens, Command::StartTls),
        Some(Verb::Login) => match (next_text(&mut tokens), next_text(&mut tokens)) {
            (Some(username), Some(password)) => Command::Login { username, password },
            _ => Command::Unsupported,
        },
        Some(Verb::Authenticate) => authenticate::parse(&mut tokens),
        Some(Verb::Append) => append::parse(tokens),
        Some(Verb::Logout) => parse_empty(tokens, Command::Logout),
        Some(Verb::Noop) => parse_empty(tokens, Command::Noop),
        Some(Verb::Idle) => parse_empty(tokens, Command::Idle),
        Some(Verb::Create) => folder::parse_create(tokens),
        Some(Verb::Delete) => folder::parse_delete(tokens),
        Some(Verb::Rename) => folder::parse_rename(tokens),
        Some(Verb::Subscribe) => folder::parse_subscribe(tokens, true),
        Some(Verb::Unsubscribe) => folder::parse_subscribe(tokens, false),
        Some(Verb::Lsub) => list::parse_lsub(tokens),
        Some(Verb::Status) => status::parse(tokens),
        Some(Verb::Check) => parse_empty(tokens, Command::Check),
        Some(Verb::Close) => parse_empty(tokens, Command::Close),
        Some(Verb::Unselect) => parse_empty(tokens, Command::Unselect),
        Some(Verb::Copy) => copy::parse(tokens, Addressing::Sequence, Transfer::Copy),
        Some(Verb::Move) => copy::parse(tokens, Addressing::Sequence, Transfer::Move),
        Some(Verb::Expunge) => parse_empty(tokens, Command::Expunge),
        Some(Verb::List) => list::parse(tokens),
        Some(Verb::Select) => parse_select(&mut tokens, false),
        Some(Verb::Examine) => parse_select(&mut tokens, true),
        Some(Verb::Fetch) => fetch::parse(&mut tokens, Addressing::Sequence),
        Some(Verb::Store) => flags::parse_store(&mut tokens, Addressing::Sequence),
        Some(Verb::Search) => search::parse(tokens, Addressing::Sequence),
        Some(Verb::Uid) => match next_atom(&mut tokens).as_deref().map(parse_verb) {
            Some(Verb::Fetch) => fetch::parse(&mut tokens, Addressing::Uid),
            Some(Verb::Store) => flags::parse_store(&mut tokens, Addressing::Uid),
            Some(Verb::Search) => search::parse(tokens, Addressing::Uid),
            Some(Verb::Copy) => copy::parse(tokens, Addressing::Uid, Transfer::Copy),
            Some(Verb::Move) => copy::parse(tokens, Addressing::Uid, Transfer::Move),
            Some(Verb::Expunge) => parse_uid_expunge(tokens),
            _ => Command::Unsupported,
        },
        Some(Verb::Unsupported) | None => Command::Unsupported,
    };
    TaggedCommand { tag, command }
}

fn parse_uid_expunge(mut tokens: impl Iterator<Item = Token>) -> Command {
    match (next_atom(&mut tokens), tokens.next()) {
        (Some(value), None) => sequence_set::MessageSet::parse(&value)
            .map(|sequence_set| Command::UidExpunge { sequence_set })
            .unwrap_or(Command::Unsupported),
        _ => Command::Unsupported,
    }
}

fn parse_select(tokens: &mut impl Iterator<Item = Token>, read_only: bool) -> Command {
    let Some(mailbox) = next_text(tokens).and_then(folder::mailbox) else {
        return Command::Unsupported;
    };
    match read_only {
        true => Command::Examine { mailbox },
        false => Command::Select { mailbox },
    }
}

fn parse_empty(mut tokens: impl Iterator<Item = Token>, command: Command) -> Command {
    match tokens.next() {
        None => command,
        Some(_) => Command::Unsupported,
    }
}

pub(super) fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) | Token::Quoted(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    }
}

fn next_atom(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    match tokens.next()? {
        Token::Text(value) => Some(value),
        Token::Quoted(_) | Token::ListStart | Token::ListEnd => None,
    }
}

fn unsupported(tag: &str) -> TaggedCommand {
    TaggedCommand {
        tag: tag.into(),
        command: Command::Unsupported,
    }
}

#[cfg(test)]
#[path = "imap/tests.rs"]
mod tests;
