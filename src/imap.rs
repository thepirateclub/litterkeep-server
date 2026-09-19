//! Декодирование IMAP-команд: текст сети преобразуется в типизированные варианты.

pub mod tokenizer;

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
        Some(Verb::Logout) => Command::Logout,
        Some(Verb::Select) => Command::Select,
        Some(Verb::Examine) => Command::Examine,
        Some(Verb::Fetch) => match (
            next_text(&mut tokens).and_then(|word| word.parse().ok()),
            next_text(&mut tokens).and_then(|word| parse_fetch_attribute(&word)),
        ) {
            (Some(sequence), Some(attribute)) => Command::Fetch {
                sequence,
                attribute,
            },
            _ => Command::Unsupported,
        },
        Some(Verb::Unsupported) | None => Command::Unsupported,
    };
    TaggedCommand { tag, command }
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
