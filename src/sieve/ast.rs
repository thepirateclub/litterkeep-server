//! AST ограниченного профиля Sieve без привязки к файловому хранилищу.

/// Проверенный script, готовый для bounded execution.
#[derive(Debug, Eq, PartialEq)]
pub struct Script {
    pub requires: Vec<String>,
    pub commands: Vec<Command>,
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
    Redirect(String),
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
