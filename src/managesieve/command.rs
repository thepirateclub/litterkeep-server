//! Incremental-framing boundary для ManageSieve command line.

/// Typed ManageSieve commands до session-state и storage owner.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Capability,
    StartTls,
    Unauthenticate,
    AuthenticatePlain(Option<String>),
    AuthenticateScramSha1(Option<String>),
    AuthenticateScramSha256(Option<String>),
    AuthenticateResponse(String),
    Logout,
    HaveSpace { name: String, size: u64 },
    PutScript { name: String, source: String },
    ListScripts,
    SetActive(Option<String>),
    GetScript(String),
    DeleteScript(String),
    RenameScript { old: String, new: String },
    CheckScript(String),
    Noop(Option<String>),
    Invalid,
}

/// Поддерживаемые и явно неподдержанные SASL mechanism tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaslMechanism {
    Plain,
    ScramSha1,
    ScramSha256,
    Unsupported,
}

/// Различает единственную wire-команду отмены от непрозрачного SASL payload.
#[derive(Debug, Eq, PartialEq)]
pub enum SaslResponse {
    Cancel,
    Payload(String),
}

impl SaslResponse {
    /// Классифицирует отмену до передачи protocol data следующему слою.
    pub fn parse(value: String) -> Self {
        match value.as_str() {
            "*" => Self::Cancel,
            _ => Self::Payload(value),
        }
    }
}

impl SaslMechanism {
    /// Сопоставляет регистронезависимый wire token с типизированным механизмом.
    pub fn parse(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "PLAIN" => Self::Plain,
            "SCRAM-SHA-1" => Self::ScramSha1,
            "SCRAM-SHA-256" => Self::ScramSha256,
            _ => Self::Unsupported,
        }
    }
}

/// Разбирает уже полный ManageSieve frame. Literal читает reactor до вызова.
pub fn parse(line: &str) -> Command {
    let mut words = match words(line) {
        Ok(words) => words.into_iter(),
        Err(()) => return Command::Invalid,
    };
    let Some(verb) = words.next() else {
        return Command::Invalid;
    };
    match verb.to_ascii_uppercase().as_str() {
        "CAPABILITY" => empty(words, Command::Capability),
        "STARTTLS" => empty(words, Command::StartTls),
        "UNAUTHENTICATE" => empty(words, Command::Unauthenticate),
        "LOGOUT" => empty(words, Command::Logout),
        "LISTSCRIPTS" => empty(words, Command::ListScripts),
        "NOOP" => optional(words, Command::Noop),
        "AUTHENTICATE" => authenticate(words),
        "HAVESPACE" => havespace(words),
        "PUTSCRIPT" => pair(words, |name, source| Command::PutScript { name, source }),
        "SETACTIVE" => set_active(words),
        "GETSCRIPT" => one(words, Command::GetScript),
        "DELETESCRIPT" => one(words, Command::DeleteScript),
        "CHECKSCRIPT" => one(words, Command::CheckScript),
        "RENAMESCRIPT" => pair(words, |old, new| Command::RenameScript { old, new }),
        _ => Command::Invalid,
    }
}

fn empty(mut words: impl Iterator<Item = String>, command: Command) -> Command {
    match words.next() {
        None => command,
        Some(_) => Command::Invalid,
    }
}
fn one(mut words: impl Iterator<Item = String>, make: fn(String) -> Command) -> Command {
    match (words.next(), words.next()) {
        (Some(value), None) => make(value),
        _ => Command::Invalid,
    }
}
fn optional(
    mut words: impl Iterator<Item = String>,
    make: fn(Option<String>) -> Command,
) -> Command {
    match (words.next(), words.next()) {
        (None, None) => make(None),
        (Some(value), None) => make(Some(value)),
        _ => Command::Invalid,
    }
}

/// Различает обязательную строку SETACTIVE и пустую строку-сигнал деактивации.
fn set_active(mut words: impl Iterator<Item = String>) -> Command {
    match (words.next(), words.next()) {
        (Some(name), None) => match name.is_empty() {
            true => Command::SetActive(None),
            false => Command::SetActive(Some(name)),
        },
        _ => Command::Invalid,
    }
}

fn pair(mut words: impl Iterator<Item = String>, make: fn(String, String) -> Command) -> Command {
    match (words.next(), words.next(), words.next()) {
        (Some(first), Some(second), None) => make(first, second),
        _ => Command::Invalid,
    }
}

fn authenticate(mut words: impl Iterator<Item = String>) -> Command {
    match (words.next(), words.next(), words.next()) {
        (Some(mechanism), response, None) => match (SaslMechanism::parse(&mechanism), response) {
            (SaslMechanism::Plain, Some(response)) => Command::AuthenticatePlain(Some(response)),
            (SaslMechanism::Plain, None) => Command::AuthenticatePlain(None),
            (SaslMechanism::ScramSha1, Some(response)) => {
                Command::AuthenticateScramSha1(Some(response))
            }
            (SaslMechanism::ScramSha1, None) => Command::AuthenticateScramSha1(None),
            (SaslMechanism::ScramSha256, Some(response)) => {
                Command::AuthenticateScramSha256(Some(response))
            }
            (SaslMechanism::ScramSha256, None) => Command::AuthenticateScramSha256(None),
            (SaslMechanism::Unsupported, _) => Command::Invalid,
        },
        _ => Command::Invalid,
    }
}

/// Разбирает отдельную quoted строку-ответ SASL, когда идёт continuation.
pub fn parse_authentication_response(line: &str) -> Option<String> {
    let mut characters = line.chars().peekable();
    match characters.next() {
        Some('"') => {}
        _ => return None,
    }
    let response = quoted(&mut characters).ok()?;
    match characters.next() {
        None => Some(response),
        Some(_) => None,
    }
}

fn havespace(mut words: impl Iterator<Item = String>) -> Command {
    match (words.next(), words.next(), words.next()) {
        (Some(name), Some(size), None) => size
            .parse()
            .map_or(Command::Invalid, |size| Command::HaveSpace { name, size }),
        _ => Command::Invalid,
    }
}

/// Quoted strings экранируют только quote и backslash, как RFC 5804 strings.
/// Разбирает атомы и quoted-string целиком, не теряя ошибочный хвост команды.
fn words(line: &str) -> Result<Vec<String>, ()> {
    let mut result = Vec::new();
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if character.is_ascii_whitespace() {
            continue;
        }
        match character {
            '"' => result.push(quoted(&mut characters)?),
            value => {
                let mut word = String::from(value);
                while let Some(next) = characters.peek().copied() {
                    if next.is_ascii_whitespace() {
                        break;
                    }
                    let _ = characters.next();
                    word.push(next);
                }
                result.push(word);
            }
        }
    }
    Ok(result)
}

fn quoted(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, ()> {
    let mut result = String::new();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Ok(result),
            '\\' => match characters.next() {
                Some(value @ ('"' | '\\')) => result.push(value),
                _ => return Err(()),
            },
            value => result.push(value),
        }
    }
    Err(())
}
