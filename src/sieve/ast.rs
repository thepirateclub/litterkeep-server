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
    Vacation,
    VacationSeconds,
    Variables,
    EncodedCharacter,
    Unsupported(String),
}

/// Поле SMTP envelope, доступное базовому Sieve test `envelope`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvelopePart {
    From,
    To,
}

/// Сохраняет envelope-part string до момента динамической оценки RFC 5229.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvelopePartSelector {
    Static(EnvelopePart),
    Variable(String),
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
    FileInto(Vec<u8>),
    FileIntoCopy(Vec<u8>),
    Redirect(Vec<u8>),
    RedirectCopy(Vec<u8>),
    Vacation(VacationAction),
    Set(SetAction),
    Stop,
}

/// Typed RFC 5229 `set` action с constant name и строкой значения.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetAction {
    pub name: String,
    pub value: String,
    pub modifiers: Vec<VariableModifier>,
}

/// RFC 5229 modifier, сохранённый в parsed `set` action.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VariableModifier {
    Lower,
    Upper,
    LowerFirst,
    UpperFirst,
    QuoteWildcard,
    Length,
}

/// Параметры RFC 5230, сохранённые без потери исходных Sieve strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VacationAction {
    pub interval: Option<VacationInterval>,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub addresses: Vec<String>,
    pub mime: bool,
    pub handle: Option<String>,
    pub reason: String,
}

/// Интервал RFC 5230 либо уточняющий интервал RFC 6131.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VacationInterval {
    Days(u64),
    Seconds(u64),
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
        parts: Vec<EnvelopePartSelector>,
        part: AddressPart,
        matcher: StringMatch,
    },
    String {
        sources: Vec<Vec<u8>>,
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
    /// RFC 5229 string-argument comparator, раскрываемый при оценке test-а.
    Variable(String),
}

/// Одна typed match-спецификация с ограниченным набором ключей.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringMatch {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub keys: Vec<Vec<u8>>,
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
