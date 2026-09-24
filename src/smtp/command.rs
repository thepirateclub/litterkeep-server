//! Типизированная грамматика SMTP команд и расширенного MAIL FROM.

/// Команда SMTP после разбора wire-строки без потери ESMTP параметров.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Ehlo(String),
    Helo(String),
    Mail(MailFrom),
    Rcpt(String),
    Data,
    /// Initial response механизма SASL PLAIN в base64-представлении.
    AuthPlain(String),
    /// Проверка имени, не раскрывающая наличие локального пользователя.
    Vrfy(String),
    /// Запрос списка участников, который локальная политика запрещает.
    Expn(String),
    /// Справка SMTP с необязательным именем команды.
    Help(Option<String>),
    StartTls,
    Noop(Option<String>),
    Rset,
    Quit,
}

/// Типизированные поля расширенной команды MAIL FROM RFC 1870.
#[derive(Debug, Eq, PartialEq)]
pub struct MailFrom {
    pub reverse_path: String,
    pub declared_size: Option<u128>,
    pub body_mode: BodyMode,
}

/// RFC 6152 body declaration; default traffic remains strict seven-bit SMTP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyMode {
    SevenBit,
    EightBitMime,
}

/// Причина отказа parser-а позволяет выбрать точный SMTP reply code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    Syntax,
    UnsupportedParameter,
    UnsupportedAuthenticationMechanism,
}

/// Локальный предел совпадает с числом, передаваемым в EHLO SIZE.
pub const MAX_MESSAGE_SIZE: u64 = 25 * 1024 * 1024;
/// Базовый предел SMTP-командной строки, включая завершающие CRLF октеты.
pub const MAX_COMMAND_LINE_SIZE: usize = 512;
/// Максимальный SMTP reverse-path/forward-path вместе с угловыми скобками.
pub const MAX_PATH_SIZE: usize = 256;

/// Преобразует одну полную CRLF строку в закрытый набор SMTP команд.
pub fn parse(line: &str) -> Result<Command, ParseError> {
    let line = line.trim_end_matches("\r\n");
    let (verb, argument) = line.split_once(' ').unwrap_or((line, ""));
    match verb.to_ascii_uppercase().as_str() {
        "EHLO" if !argument.is_empty() => Ok(Command::Ehlo(argument.into())),
        "HELO" if !argument.is_empty() => Ok(Command::Helo(argument.into())),
        "MAIL" => parse_mail_from(argument).map(Command::Mail),
        "RCPT" => parse_recipient(argument).map(Command::Rcpt),
        "DATA" if argument.is_empty() => Ok(Command::Data),
        "AUTH" => parse_auth(argument),
        "VRFY" => parse_string_command(argument).map(Command::Vrfy),
        "EXPN" => parse_string_command(argument).map(Command::Expn),
        "HELP" => parse_optional_string(argument).map(Command::Help),
        "STARTTLS" if argument.is_empty() => Ok(Command::StartTls),
        "NOOP" => parse_optional_string(argument).map(Command::Noop),
        "RSET" if argument.is_empty() => Ok(Command::Rset),
        "QUIT" if argument.is_empty() => Ok(Command::Quit),
        _ => Err(ParseError::Syntax),
    }
}

/// Проверяет RFC 5321 printable ASCII string без CRLF и пустого аргумента.
fn parse_string_command(argument: &str) -> Result<String, ParseError> {
    let value = argument.trim();
    let valid = !value.is_empty() && value.bytes().all(|byte| (32..=126).contains(&byte));
    match valid {
        true => Ok(value.to_owned()),
        false => Err(ParseError::Syntax),
    }
}

/// Разбирает необязательный параметр HELP/NOOP без изменения его значения.
fn parse_optional_string(argument: &str) -> Result<Option<String>, ParseError> {
    match argument.trim().is_empty() {
        true => Ok(None),
        false => parse_string_command(argument).map(Some),
    }
}

/// Распознаёт только реализованный SASL механизм.
fn parse_auth(argument: &str) -> Result<Command, ParseError> {
    let Some((mechanism, response)) = argument.split_once(' ') else {
        return Err(ParseError::Syntax);
    };
    match mechanism.to_ascii_uppercase().as_str() {
        "PLAIN" if !response.is_empty() => Ok(Command::AuthPlain(response.to_owned())),
        _ => Err(ParseError::UnsupportedAuthenticationMechanism),
    }
}

/// Разбирает reverse-path и ровно один необязательный SIZE параметр.
fn parse_mail_from(argument: &str) -> Result<MailFrom, ParseError> {
    let (reverse_path, suffix) = parse_path(argument, "FROM:", true)?;
    let parsed_parameters = suffix
        .split_ascii_whitespace()
        .map(parse_mail_parameter)
        .collect::<Result<Vec<_>, _>>()?;
    let mut declared_size = None;
    let mut body_mode = BodyMode::SevenBit;
    let mut size_seen = false;
    let mut body_seen = false;
    for parameter in parsed_parameters {
        match parameter {
            MailParameter::Size(size) if !size_seen => {
                declared_size = Some(size);
                size_seen = true;
            }
            MailParameter::Body(mode) if !body_seen => {
                body_mode = mode;
                body_seen = true;
            }
            MailParameter::Size(_) | MailParameter::Body(_) => return Err(ParseError::Syntax),
        }
    }
    Ok(MailFrom {
        reverse_path,
        declared_size,
        body_mode,
    })
}

/// MAIL не рекламирует иные расширения, а RCPT пока принимает только path.
fn parse_recipient(argument: &str) -> Result<String, ParseError> {
    let (recipient, suffix) = parse_path(argument, "TO:", false)?;
    match suffix.is_empty() {
        true => Ok(recipient),
        false => Err(ParseError::UnsupportedParameter),
    }
}

/// Выделяет angle path и оставляет хвост ESMTP параметров для typed parser-а.
fn parse_path<'a>(
    argument: &'a str,
    prefix: &str,
    accepts_empty_path: bool,
) -> Result<(String, &'a str), ParseError> {
    let (actual, rest) = argument.split_once(':').ok_or(ParseError::Syntax)?;
    let expected = prefix.strip_suffix(':').unwrap_or(prefix);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(ParseError::Syntax);
    }
    let value = rest.trim_start();
    let Some(path) = value.strip_prefix('<') else {
        return Err(ParseError::Syntax);
    };
    let Some((mailbox, suffix)) = path.split_once('>') else {
        return Err(ParseError::Syntax);
    };
    if mailbox.len().saturating_add(2) > MAX_PATH_SIZE {
        return Err(ParseError::Syntax);
    }
    if mailbox.is_empty() && !accepts_empty_path {
        return Err(ParseError::Syntax);
    }
    match suffix.is_empty() {
        true => Ok((mailbox.to_owned(), "")),
        false if suffix.chars().next().is_some_and(char::is_whitespace) => {
            Ok((mailbox.to_owned(), suffix.trim()))
        }
        false => Err(ParseError::Syntax),
    }
}

/// Преобразует ESMTP parameter keyword в явно поддерживаемый enum.
fn parse_mail_parameter(parameter: &str) -> Result<MailParameter, ParseError> {
    let (name, value) = parameter.split_once('=').ok_or(ParseError::Syntax)?;
    match name.to_ascii_uppercase().as_str() {
        "SIZE" => parse_size(value).map(MailParameter::Size),
        "BODY" => parse_body_mode(value).map(MailParameter::Body),
        _ => Err(ParseError::UnsupportedParameter),
    }
}

/// Преобразует BODY в перечисление только после EHLO-поддержки 8BITMIME.
fn parse_body_mode(value: &str) -> Result<BodyMode, ParseError> {
    match value.to_ascii_uppercase().as_str() {
        "7BIT" => Ok(BodyMode::SevenBit),
        "8BITMIME" => Ok(BodyMode::EightBitMime),
        _ => Err(ParseError::Syntax),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MailParameter {
    Size(u128),
    Body(BodyMode),
}

/// Ограничивает SIZE value диапазоном RFC 1870: от 1 до 20 цифр.
fn parse_size(value: &str) -> Result<u128, ParseError> {
    let valid = (1..=20).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit());
    match valid {
        true => value.parse().map_err(|_| ParseError::Syntax),
        false => Err(ParseError::Syntax),
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
