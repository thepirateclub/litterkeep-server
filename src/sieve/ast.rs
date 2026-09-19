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
    HeaderContains { name: String, needle: String },
    Exists { names: Vec<String> },
    Not(Box<Condition>),
    AllOf(Vec<Condition>),
    AnyOf(Vec<Condition>),
}
