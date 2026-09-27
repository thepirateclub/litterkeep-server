//! Grammar POP3 command line до доступа к состоянию session или Store.

/// Типизированные POP3 verbs без сохранения протокольных строк в FSM.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    User(String),
    Pass(String),
    Stat,
    List(Option<u32>),
    Retr(u32),
    Dele(u32),
    Noop,
    Rset,
    Quit,
    Uidl(Option<u32>),
    Top {
        number: u32,
        lines: u32,
    },
    Capa,
    Stls,
    /// Включает режим UTF-8 сообщений по RFC 6856 без изменения credentials.
    Utf8,
    /// Согласует язык протокольных ответов по RFC 6856.
    Lang(super::LanguageRequest),
    Auth {
        mechanism: SaslMechanism,
        initial_response: Option<SaslInitialResponse>,
    },
    Invalid,
}

/// Поддерживаемый набор механизмов POP3 SASL.
#[derive(Debug, Eq, PartialEq)]
pub enum SaslMechanism {
    Plain,
    Unsupported,
}

/// Различает отсутствие ответа, пустой SASL response и переданные Base64 данные.
#[derive(Debug, Eq, PartialEq)]
pub enum SaslInitialResponse {
    Empty,
    Encoded(String),
}

/// RFC 2449 ограничивает команду вместе с CRLF 255 октетами.
pub const MAX_COMMAND_LINE_SIZE: usize = 255;
/// RFC 1939 ограничивает аргументы USER/PASS сорока печатными ASCII октетами.
const MAX_CREDENTIAL_ARGUMENT_SIZE: usize = 40;

/// Единственная граница POP3 command line и её точной arity-проверки.
pub fn parse(line: &str) -> Command {
    let Some(command) = line.strip_suffix("\r\n") else {
        return Command::Invalid;
    };
    if line.len() > MAX_COMMAND_LINE_SIZE || !printable_command(command) {
        return Command::Invalid;
    }
    let mut words = command.split(' ');
    let verb = words.next().unwrap_or_default().to_ascii_uppercase();
    if !(3..=4).contains(&verb.len()) {
        return Command::Invalid;
    }
    match verb.as_str() {
        "USER" => credential(words.next(), words.next()).map_or(Command::Invalid, Command::User),
        "PASS" => pass_argument(command).map_or(Command::Invalid, Command::Pass),
        "STAT" => empty(words.next(), Command::Stat),
        "LIST" => optional_number(words.next(), words.next(), Command::List),
        "RETR" => required_number(words.next(), words.next(), Command::Retr),
        "DELE" => required_number(words.next(), words.next(), Command::Dele),
        "NOOP" => empty(words.next(), Command::Noop),
        "RSET" => empty(words.next(), Command::Rset),
        "QUIT" => empty(words.next(), Command::Quit),
        "UIDL" => optional_number(words.next(), words.next(), Command::Uidl),
        "TOP" => top(words.next(), words.next(), words.next()),
        "CAPA" => empty(words.next(), Command::Capa),
        "STLS" => empty(words.next(), Command::Stls),
        "UTF8" => empty(words.next(), Command::Utf8),
        "LANG" => super::language::parse(words.next(), words.next())
            .map_or(Command::Invalid, Command::Lang),
        "AUTH" => auth(words.next(), words.next(), words.next()),
        _ => Command::Invalid,
    }
}

/// Разбирает AUTH с необязательным initial response в пределах command line.
fn auth(mechanism: Option<&str>, initial_response: Option<&str>, extra: Option<&str>) -> Command {
    match (mechanism, initial_response, extra) {
        (Some(name), None, None) if !name.is_empty() => Command::Auth {
            mechanism: sasl_mechanism(name),
            initial_response: None,
        },
        (Some(name), Some(response), None) if !name.is_empty() && !response.is_empty() => {
            Command::Auth {
                mechanism: sasl_mechanism(name),
                initial_response: Some(parse_initial_response(response)),
            }
        }
        _ => Command::Invalid,
    }
}

/// Переводит RFC 5034 маркер пустого ответа в typed состояние до SASL FSM.
fn parse_initial_response(response: &str) -> SaslInitialResponse {
    match response.as_bytes() {
        b"=" => SaslInitialResponse::Empty,
        _ => SaslInitialResponse::Encoded(response.to_owned()),
    }
}

/// Сопоставляет SASL mechanism case-insensitively с закрытым enum.
fn sasl_mechanism(name: &str) -> SaslMechanism {
    match name.to_ascii_uppercase().as_str() {
        "PLAIN" => SaslMechanism::Plain,
        _ => SaslMechanism::Unsupported,
    }
}

/// POP3 keywords и параметры используют печатный ASCII и один SP separator.
fn printable_command(command: &str) -> bool {
    command
        .bytes()
        .all(|octet| octet == b' ' || (33..=126).contains(&octet))
}

fn text(value: Option<&str>, extra: Option<&str>) -> Option<String> {
    match (value, extra) {
        (Some(value), None) if !value.is_empty() => Some(value.to_owned()),
        _ => None,
    }
}

/// Ограничивает единственный ASCII credential аргумент пределом RFC 1939.
fn credential(value: Option<&str>, extra: Option<&str>) -> Option<String> {
    text(value, extra).filter(|argument| argument.len() <= MAX_CREDENTIAL_ARGUMENT_SIZE)
}

/// Сохраняет пробелы в единственном аргументе PASS согласно RFC 1939 §7.3.
fn pass_argument(command: &str) -> Option<String> {
    let (verb, password) = command.split_once(' ')?;
    match (
        verb.eq_ignore_ascii_case("PASS"),
        password.is_empty(),
        password.len() <= MAX_CREDENTIAL_ARGUMENT_SIZE,
    ) {
        (true, false, true) => Some(password.to_owned()),
        _ => None,
    }
}
fn empty(extra: Option<&str>, command: Command) -> Command {
    match extra {
        None => command,
        Some(_) => Command::Invalid,
    }
}
fn optional_number(
    value: Option<&str>,
    extra: Option<&str>,
    construct: impl FnOnce(Option<u32>) -> Command,
) -> Command {
    match (value, extra) {
        (None, None) => construct(None),
        (Some(value), None) => value
            .parse()
            .ok()
            .map_or(Command::Invalid, |number| construct(Some(number))),
        _ => Command::Invalid,
    }
}
fn required_number(
    value: Option<&str>,
    extra: Option<&str>,
    construct: impl FnOnce(u32) -> Command,
) -> Command {
    match (value, extra) {
        (Some(value), None) => value.parse().ok().map_or(Command::Invalid, construct),
        _ => Command::Invalid,
    }
}
fn top(number: Option<&str>, lines: Option<&str>, extra: Option<&str>) -> Command {
    match (
        number.and_then(|value| value.parse().ok()),
        lines.and_then(|value| value.parse().ok()),
        extra,
    ) {
        (Some(number), Some(lines), None) => Command::Top { number, lines },
        _ => Command::Invalid,
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
