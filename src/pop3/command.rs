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
    Top { number: u32, lines: u32 },
    Capa,
    Stls,
    Invalid,
}

/// RFC 2449 ограничивает команду вместе с CRLF 255 октетами.
pub const MAX_COMMAND_LINE_SIZE: usize = 255;

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
        "USER" => text(words.next(), words.next()).map_or(Command::Invalid, Command::User),
        "PASS" => text(words.next(), words.next()).map_or(Command::Invalid, Command::Pass),
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
        _ => Command::Invalid,
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
        (Some(value), None) => Some(value.to_owned()),
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
