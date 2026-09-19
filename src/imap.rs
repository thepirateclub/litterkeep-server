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
    StartTls,
    Login {
        username: String,
        password: String,
    },
    Logout,
    Select,
    Examine,
    Fetch {
        sequence: usize,
        attribute: FetchAttribute,
    },
    Unsupported,
}

/// Поддерживаемый одиночный атрибут FETCH начального профиля.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchAttribute {
    Rfc822,
    Rfc822Size,
    BodyStructure,
    Envelope,
    Body {
        peek: bool,
        section: BodySection,
        partial: Option<Partial>,
    },
}

/// Выбираемая raw MIME section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodySection {
    EntireMessage,
    Part {
        path: Vec<usize>,
        mime_headers: bool,
    },
}

/// Октетный диапазон IMAP partial fetch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Partial {
    pub offset: usize,
    pub count: usize,
}

/// Внутренний verb до проверки аргументов команды.
enum Verb {
    Capability,
    StartTls,
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
        Verb::StartTls => Command::StartTls,
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
        Verb::Fetch => match (
            words.next().and_then(|word| word.parse().ok()),
            words.next().and_then(parse_fetch_attribute),
        ) {
            (Some(sequence), Some(attribute)) => Command::Fetch {
                sequence,
                attribute,
            },
            _ => Command::Unsupported,
        },
        Verb::Unsupported => Command::Unsupported,
    };
    TaggedCommand { tag, command }
}

fn parse_fetch_attribute(value: &str) -> Option<FetchAttribute> {
    let token = value.trim_matches(['(', ')']);
    match token.to_ascii_uppercase().as_str() {
        "RFC822" => Some(FetchAttribute::Rfc822),
        "RFC822.SIZE" => Some(FetchAttribute::Rfc822Size),
        "BODYSTRUCTURE" => Some(FetchAttribute::BodyStructure),
        "ENVELOPE" => Some(FetchAttribute::Envelope),
        _ => parse_body_attribute(token),
    }
}

fn parse_body_attribute(token: &str) -> Option<FetchAttribute> {
    let upper = token.to_ascii_uppercase();
    let (peek, after_prefix) = match (
        upper.strip_prefix("BODY.PEEK["),
        upper.strip_prefix("BODY["),
    ) {
        (Some(value), _) => (true, value),
        (None, Some(value)) => (false, value),
        (None, None) => return None,
    };
    let (section_text, suffix) = after_prefix.split_once(']')?;
    let section = parse_body_section(section_text)?;
    let partial = match suffix {
        "" => None,
        value => Some(parse_partial(value)?),
    };
    Some(FetchAttribute::Body {
        peek,
        section,
        partial,
    })
}

fn parse_body_section(value: &str) -> Option<BodySection> {
    if value.is_empty() {
        return Some(BodySection::EntireMessage);
    }
    let mut path = Vec::new();
    let mut mime_headers = false;
    for component in value.split('.') {
        match component {
            "MIME" => mime_headers = true,
            number => path.push(number.parse::<usize>().ok()?),
        }
    }
    if path.is_empty() {
        return None;
    }
    Some(BodySection::Part { path, mime_headers })
}

fn parse_partial(value: &str) -> Option<Partial> {
    let inner = value.strip_prefix('<')?.strip_suffix('>')?;
    let (offset, count) = inner.split_once('.')?;
    Some(Partial {
        offset: offset.parse().ok()?,
        count: count.parse().ok()?,
    })
}

/// Удаляет парные кавычки IMAP atom, не меняя неквотированное значение.
fn atom(value: &str) -> String {
    value.trim_matches('"').into()
}

/// Единственная допустимая граница строкового сравнения протокольного verb.
fn parse_verb(text: &str) -> Verb {
    match text.to_ascii_uppercase().as_str() {
        "CAPABILITY" => Verb::Capability,
        "STARTTLS" => Verb::StartTls,
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

    /// STARTTLS преобразуется в enum до входа в session FSM.
    #[test]
    fn starttls_is_typed() {
        assert_eq!(
            parse("a2 STARTTLS\r\n"),
            TaggedCommand {
                tag: "a2".into(),
                command: Command::StartTls,
            }
        );
    }

    #[test]
    fn fetch_attribute_is_typed() {
        assert_eq!(
            parse("a3 FETCH 2 (BODYSTRUCTURE)\r\n").command,
            Command::Fetch {
                sequence: 2,
                attribute: FetchAttribute::BodyStructure,
            }
        );
    }

    #[test]
    fn body_peek_partial_is_typed() {
        assert_eq!(
            parse("a4 FETCH 1 (BODY.PEEK[2.MIME]<3.7>)\r\n").command,
            Command::Fetch {
                sequence: 1,
                attribute: FetchAttribute::Body {
                    peek: true,
                    section: BodySection::Part {
                        path: vec![2],
                        mime_headers: true,
                    },
                    partial: Some(Partial {
                        offset: 3,
                        count: 7
                    }),
                },
            }
        );
    }
}
