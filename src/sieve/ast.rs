//! AST ограниченного профиля Sieve без привязки к файловому хранилищу.

/// Проверенный script, готовый для bounded execution.
#[derive(Debug, Eq, PartialEq)]
pub struct Script {
    pub requires: Vec<Capability>,
    pub commands: Vec<Command>,
}

/// Возможность, заявленная script-ом до static validation.
#[derive(Debug, Eq, PartialEq)]
pub enum Capability {
    FileInto,
    Copy,
    Subaddress,
    Envelope,
    Unsupported(String),
}

/// Поле SMTP envelope, доступное базовому Sieve test `envelope`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopePart {
    From,
    To,
}

/// Верхнеуровневая команда языка.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Action(Action),
    If {
        condition: Condition,
        then_commands: Vec<Command>,
        else_commands: Vec<Command>,
    },
}

/// Действие не пишет в Store, а описывает намерение delivery transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Keep,
    Discard,
    FileInto(String),
    FileIntoCopy(String),
    Redirect(String),
    RedirectCopy(String),
    Stop,
}

/// Предикат, вычисляемый исключительно на MIME headers.
#[derive(Debug, Eq, PartialEq)]
pub enum Condition {
    True,
    False,
    Header {
        names: Vec<String>,
        matcher: StringMatch,
    },
    Address {
        names: Vec<String>,
        part: AddressPart,
        matcher: StringMatch,
    },
    Envelope {
        parts: Vec<EnvelopePart>,
        part: AddressPart,
        matcher: StringMatch,
    },
    Exists {
        names: Vec<String>,
    },
    Size {
        relation: SizeRelation,
        limit: usize,
    },
    Not(Box<Condition>),
    AllOf(Vec<Condition>),
    AnyOf(Vec<Condition>),
}

/// Тип сравнения строк, выбранный в Sieve-скрипте до исполнения.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchType {
    Is,
    Contains,
    Matches,
}

/// Явная политика сравнения, не зависящая от locale сервера.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Comparator {
    AsciiCasemap,
    Octet,
}

/// Одна typed match-спецификация с ограниченным набором ключей.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringMatch {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub keys: Vec<String>,
}

/// Направление сравнения размера исходного сообщения в октетах.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SizeRelation {
    Over,
    Under,
}

/// Выбираемая Sieve-проекция mailbox из address header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressPart {
    All,
    LocalPart,
    Domain,
    User,
    Detail,
}
