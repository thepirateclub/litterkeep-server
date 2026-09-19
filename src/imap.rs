//! Декодирование IMAP-команд: текст сети преобразуется в типизированные варианты.

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
    Login { username: String, password: String },
    Logout,
    Select,
    Examine,
    Fetch(usize),
    Unsupported,
}

/// Внутренний verb до проверки аргументов команды.
enum Verb {
    Capability,
    Login,
    Logout,
    Select,
    Examine,
    Fetch,
    Unsupported,
}

/// Разбирает одну CRLF-строку и не оставляет строковые команды в reactor.
pub fn parse(line: &str) -> TaggedCommand {
    let mut words = line.split_whitespace();
    let tag = words.next().unwrap_or("*").into();
    let command = match parse_verb(words.next().unwrap_or("")) {
        Verb::Capability => Command::Capability,
        Verb::Login => match (words.next(), words.next()) {
            (Some(username), Some(password)) => Command::Login {
                username: atom(username),
                password: atom(password),
            },
            _ => Command::Unsupported,
        },
        Verb::Logout => Command::Logout,
        Verb::Select => Command::Select,
        Verb::Examine => Command::Examine,
        Verb::Fetch => match words.next().and_then(|word| word.parse().ok()) {
            Some(number) => Command::Fetch(number),
            None => Command::Unsupported,
        },
        Verb::Unsupported => Command::Unsupported,
    };
    TaggedCommand { tag, command }
}

/// Удаляет парные кавычки IMAP atom, не меняя неквотированное значение.
fn atom(value: &str) -> String {
    value.trim_matches('"').into()
}

/// Единственная допустимая граница строкового сравнения протокольного verb.
fn parse_verb(text: &str) -> Verb {
    match text.to_ascii_uppercase().as_str() {
        "CAPABILITY" => Verb::Capability,
        "LOGIN" => Verb::Login,
        "LOGOUT" => Verb::Logout,
        "SELECT" => Verb::Select,
        "EXAMINE" => Verb::Examine,
        "FETCH" => Verb::Fetch,
        _ => Verb::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Проверяет передачу реквизитов в типизированном варианте.
    #[test]
    fn login_has_credentials() {
        assert_eq!(
            parse("a LOGIN bob pass\r\n"),
            TaggedCommand {
                tag: "a".into(),
                command: Command::Login {
                    username: "bob".into(),
                    password: "pass".into()
                }
            }
        );
    }
}
