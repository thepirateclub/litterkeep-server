//! Декодирование IMAP-команд: текст сети преобразуется в типизированные варианты.

pub mod append;
pub mod copy;
pub mod fetch;
pub mod flags;
pub mod folder;
pub mod idle;
pub mod modified_utf7;
pub mod search;
pub mod sequence_set;
pub mod status;
pub mod tokenizer;

#[cfg(test)]
#[path = "imap/copy_tests.rs"]
mod copy_tests;

pub use fetch::{BodySection, FetchAttribute, Partial};
pub use idle::{IdleContinuation, parse_idle_continuation};
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
    StartTls,
    Login {
        username: String,
        password: String,
    },
    Append {
        mailbox: MailboxName,
        literal_size: usize,
        flags: crate::mail_flags::MessageFlags,
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
    Copy {
        transfer: Transfer,
        addressing: Addressing,
        sequence_set: SequenceSet,
        destination: MailboxName,
    },
    Expunge,
    UidExpunge {
        sequence_set: SequenceSet,
    },
    List {
        pattern: folder::ListPattern,
    },
    ListSpecial {
        pattern: folder::ListPattern,
    },
    Select {
        mailbox: MailboxName,
    },
    Examine {
        mailbox: MailboxName,
    },
    Fetch {
        addressing: Addressing,
        sequence_set: SequenceSet,
        attribute: FetchAttribute,
    },
    Store {
        addressing: Addressing,
        sequence_set: SequenceSet,
        mutation: crate::mail_flags::FlagMutation,
        silent: bool,
        flags: crate::mail_flags::MessageFlags,
    },
    Search {
        addressing: Addressing,
        key: search::SearchKey,
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
/// Внутренний verb до проверки аргументов команды.
enum Verb {
    Capability,
    StartTls,
    Login,
    Append,
    Logout,
    Noop,
    Idle,
    Create,
    Delete,
    Rename,
    Subscribe,
    Unsubscribe,
    Lsub,
    Status,
    Check,
    Close,
    Copy,
    Move,
    Expunge,
    List,
    Select,
    Examine,
    Fetch,
    Store,
    Search,
    Uid,
    Unsupported,
}

/// Разбирает одну CRLF-строку и не оставляет строковые команды в reactor.
pub fn parse(line: &str) -> TaggedCommand {
    let mut tokens = match tokenizer::tokenize(line) {
        Ok(tokens) => tokens.into_iter(),
        Err(_) => return unsupported("*"),
    };
    let tag = next_text(&mut tokens).unwrap_or_else(|| "*".into());
    let command = match next_text(&mut tokens).as_deref().map(parse_verb) {
        Some(Verb::Capability) => Command::Capability,
        Some(Verb::StartTls) => Command::StartTls,
        Some(Verb::Login) => match (next_text(&mut tokens), next_text(&mut tokens)) {
            (Some(username), Some(password)) => Command::Login { username, password },
            _ => Command::Unsupported,
        },
        Some(Verb::Append) => append::parse(tokens),
        Some(Verb::Logout) => Command::Logout,
        Some(Verb::Noop) => Command::Noop,
        Some(Verb::Idle) => Command::Idle,
        Some(Verb::Create) => folder::parse_create(tokens),
        Some(Verb::Delete) => folder::parse_delete(tokens),
        Some(Verb::Rename) => folder::parse_rename(tokens),
        Some(Verb::Subscribe) => folder::parse_subscribe(tokens, true),
        Some(Verb::Unsubscribe) => folder::parse_subscribe(tokens, false),
        Some(Verb::Lsub) => folder::parse_lsub(tokens),
        Some(Verb::Status) => status::parse(tokens),
        Some(Verb::Check) => parse_empty(tokens, Command::Check),
        Some(Verb::Close) => parse_empty(tokens, Command::Close),
        Some(Verb::Copy) => copy::parse(tokens, Addressing::Sequence, Transfer::Copy),
        Some(Verb::Move) => copy::parse(tokens, Addressing::Sequence, Transfer::Move),
        Some(Verb::Expunge) => parse_empty(tokens, Command::Expunge),
        Some(Verb::List) => folder::parse_list(tokens),
        Some(Verb::Select) => parse_select(&mut tokens, false),
        Some(Verb::Examine) => parse_select(&mut tokens, true),
        Some(Verb::Fetch) => fetch::parse(&mut tokens, Addressing::Sequence),
        Some(Verb::Store) => flags::parse_store(&mut tokens, Addressing::Sequence),
        Some(Verb::Search) => search::parse(tokens, Addressing::Sequence),
        Some(Verb::Uid) => match next_text(&mut tokens).as_deref().map(parse_verb) {
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
    match (next_text(&mut tokens), tokens.next()) {
        (Some(value), None) => SequenceSet::parse(&value)
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

fn next_text(tokens: &mut impl Iterator<Item = Token>) -> Option<String> {
    tokens.find_map(|token| match token {
        Token::Text(value) => Some(value),
        Token::ListStart | Token::ListEnd => None,
    })
}

fn unsupported(tag: &str) -> TaggedCommand {
    TaggedCommand {
        tag: tag.into(),
        command: Command::Unsupported,
    }
}

/// Единственная допустимая граница строкового сравнения протокольного verb.
fn parse_verb(text: &str) -> Verb {
    match text.to_ascii_uppercase().as_str() {
        "CAPABILITY" => Verb::Capability,
        "STARTTLS" => Verb::StartTls,
        "LOGIN" => Verb::Login,
        "APPEND" => Verb::Append,
        "LOGOUT" => Verb::Logout,
        "NOOP" => Verb::Noop,
        "IDLE" => Verb::Idle,
        "CREATE" => Verb::Create,
        "DELETE" => Verb::Delete,
        "RENAME" => Verb::Rename,
        "SUBSCRIBE" => Verb::Subscribe,
        "UNSUBSCRIBE" => Verb::Unsubscribe,
        "LSUB" => Verb::Lsub,
        "STATUS" => Verb::Status,
        "CHECK" => Verb::Check,
        "CLOSE" => Verb::Close,
        "COPY" => Verb::Copy,
        "MOVE" => Verb::Move,
        "EXPUNGE" => Verb::Expunge,
        "LIST" => Verb::List,
        "SELECT" => Verb::Select,
        "EXAMINE" => Verb::Examine,
        "FETCH" => Verb::Fetch,
        "STORE" => Verb::Store,
        "SEARCH" => Verb::Search,
        "UID" => Verb::Uid,
        _ => Verb::Unsupported,
    }
}

#[cfg(test)]
#[path = "imap/tests.rs"]
mod tests;
