//! Типизированное распознавание нечувствительных к регистру токенов Sieve.

/// Команды базового языка, распознаваемые до разбора аргументов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommandWord {
    Require,
    If,
    Else,
    Elsif,
    Keep,
    Discard,
    Stop,
    FileInto,
    Redirect,
    Vacation,
    Unknown,
}

impl CommandWord {
    /// Переводит atom в enum, сохраняя сравнение строк только на wire-границе.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "require" => Self::Require,
            "if" => Self::If,
            "else" => Self::Else,
            "elsif" => Self::Elsif,
            "keep" => Self::Keep,
            "discard" => Self::Discard,
            "stop" => Self::Stop,
            "fileinto" => Self::FileInto,
            "redirect" => Self::Redirect,
            "vacation" => Self::Vacation,
            _ => Self::Unknown,
        }
    }
}

/// Базовые тесты, принимаемые условным выражением Sieve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestWord {
    True,
    False,
    Header,
    Address,
    Envelope,
    Exists,
    Size,
    Not,
    AllOf,
    AnyOf,
    Unknown,
}

impl TestWord {
    /// Разбирает имя теста без учета ASCII-регистра.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "true" => Self::True,
            "false" => Self::False,
            "header" => Self::Header,
            "address" => Self::Address,
            "envelope" => Self::Envelope,
            "exists" => Self::Exists,
            "size" => Self::Size,
            "not" => Self::Not,
            "allof" => Self::AllOf,
            "anyof" => Self::AnyOf,
            _ => Self::Unknown,
        }
    }
}

/// Параметры, чьи значения являются именами comparator-ов.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ComparatorWord {
    AsciiCasemap,
    Octet,
    Unknown,
}

/// Объявляемые расширения базового обработчика Sieve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CapabilityWord {
    FileInto,
    Copy,
    Subaddress,
    Envelope,
    Vacation,
    VacationSeconds,
    Unknown,
}

impl CapabilityWord {
    /// Распознаёт идентификатор расширения, не меняя исходное неизвестное имя.
    pub(super) fn parse(value: &str) -> Self {
        // RFC 5228 §6.1 делает capability string регистрозависимым.
        match value {
            "fileinto" => Self::FileInto,
            "copy" => Self::Copy,
            "subaddress" => Self::Subaddress,
            "envelope" => Self::Envelope,
            "vacation" => Self::Vacation,
            "vacation-seconds" => Self::VacationSeconds,
            _ => Self::Unknown,
        }
    }
}

/// Теги сопоставления строковых тестов RFC 5228.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MatchTag {
    Is,
    Contains,
    Matches,
    Comparator,
    Unknown,
}

/// Tagged argument RFC 3894 :copy для delivery actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CopyTag {
    Copy,
    Unknown,
}

impl CopyTag {
    /// Разбирает тег без учёта ASCII-регистра, как требует грамматика Sieve.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            ":copy" => Self::Copy,
            _ => Self::Unknown,
        }
    }
}

impl MatchTag {
    /// Переводит распознанный atom-тег в типизированный вариант.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            ":is" => Self::Is,
            ":contains" => Self::Contains,
            ":matches" => Self::Matches,
            ":comparator" => Self::Comparator,
            _ => Self::Unknown,
        }
    }
}

/// Тег направления сравнения для теста размера.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SizeTag {
    Over,
    Under,
    Unknown,
}

/// Дополнительные части адреса из RFC 5229 subaddress extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AddressTag {
    All,
    LocalPart,
    Domain,
    User,
    Detail,
    Is,
    Contains,
    Matches,
    Comparator,
    Unknown,
}

impl AddressTag {
    /// Переводит address-test тег в enum до построения AST.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            ":all" => Self::All,
            ":localpart" => Self::LocalPart,
            ":domain" => Self::Domain,
            ":user" => Self::User,
            ":detail" => Self::Detail,
            ":is" => Self::Is,
            ":contains" => Self::Contains,
            ":matches" => Self::Matches,
            ":comparator" => Self::Comparator,
            _ => Self::Unknown,
        }
    }
}

impl SizeTag {
    /// Разбирает только регистронезависимые синтаксические теги.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            ":over" => Self::Over,
            ":under" => Self::Under,
            _ => Self::Unknown,
        }
    }
}

impl ComparatorWord {
    /// Сопоставляет идентификатор comparator-а по правилам ASCII-регистра.
    pub(super) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "i;ascii-casemap" => Self::AsciiCasemap,
            "i;octet" => Self::Octet,
            _ => Self::Unknown,
        }
    }
}
