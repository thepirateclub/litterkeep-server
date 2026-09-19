//! Декодирование IMAP-команд: текст сети преобразуется в типизированные варианты.

pub mod sequence_set;
pub mod tokenizer;

use sequence_set::SequenceSet;
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
    Select {
        mailbox: MailboxName,
    },
    Examine {
        mailbox: MailboxName,
    },
    Fetch {
        addressing: Addressing,
        sequence_set: SequenceSet,
        attribute: FetchAttribute,
    },
    Unsupported,
}

/// Типизированное имя mailbox после обработки специального имени INBOX.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailboxName {
    Inbox,
    Named(String),
}

/// Пространство номеров, в котором интерпретируется sequence-set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Addressing {
    Sequence,
    Uid,
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
    Uid,
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
        Some(Verb::Select) => parse_select(&mut tokens, false),
        Some(Verb::Examine) => parse_select(&mut tokens, true),
        Some(Verb::Fetch) => parse_fetch(&mut tokens, Addressing::Sequence),
        Some(Verb::Uid) => match next_text(&mut tokens).as_deref().map(parse_verb) {
            Some(Verb::Fetch) => parse_fetch(&mut tokens, Addressing::Uid),
            _ => Command::Unsupported,
        },
        Some(Verb::Unsupported) | None => Command::Unsupported,
    };
    TaggedCommand { tag, command }
}

fn parse_select(tokens: &mut impl Iterator<Item = Token>, read_only: bool) -> Command {
    let Some(mailbox) = next_text(tokens).map(|value| match value.to_ascii_uppercase().as_str() {
        "INBOX" => MailboxName::Inbox,
        _ => MailboxName::Named(value),
    }) else {
        return Command::Unsupported;
    };
    match read_only {
        true => Command::Examine { mailbox },
        false => Command::Select { mailbox },
    }
}

fn parse_fetch(tokens: &mut impl Iterator<Item = Token>, addressing: Addressing) -> Command {
    match (
        next_text(tokens).and_then(|word| SequenceSet::parse(&word).ok()),
        next_text(tokens).and_then(|word| parse_fetch_attribute(&word)),
    ) {
        (Some(sequence_set), Some(attribute)) => Command::Fetch {
            addressing,
            sequence_set,
            attribute,
        },
        _ => Command::Unsupported,
    }
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
        "UID" => Verb::Uid,
        _ => Verb::Unsupported,
    }
}

#[cfg(test)]
#[path = "imap/tests.rs"]
mod tests;
