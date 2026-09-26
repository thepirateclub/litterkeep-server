//! Типизированная грамматика SMTP команд и расширенного MAIL FROM.

#[path = "address.rs"]
mod address;
#[path = "address_literal.rs"]
mod address_literal;
#[path = "greeting.rs"]
mod greeting;
pub(crate) use address::canonical_parts;
#[cfg(target_os = "linux")]
pub(crate) use address::requires_smtp_utf8;

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
    /// Клиент объявил UTF-8 в envelope или заголовках сообщения.
    pub smtp_utf8: bool,
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
/// RFC 6531 добавляет десять октетов для ESMTP MAIL с SMTPUTF8.
pub const MAX_SMTPUTF8_MAIL_LINE_SIZE: usize = MAX_COMMAND_LINE_SIZE + 10;
/// Максимальный SMTP reverse-path/forward-path вместе с угловыми скобками.
pub const MAX_PATH_SIZE: usize = 256;
/// Максимальный размер SMTP local-part в октетах согласно RFC 5321.
pub const MAX_LOCAL_PART_SIZE: usize = 64;

/// Преобразует одну полную CRLF строку в закрытый набор SMTP команд.
pub fn parse(line: &str) -> Result<Command, ParseError> {
    let line = line.trim_end_matches("\r\n");
    let (verb, argument) = line.split_once(' ').unwrap_or((line, ""));
    match verb.to_ascii_uppercase().as_str() {
        "EHLO" => greeting::ehlo(argument).map(Command::Ehlo),
        "HELO" => greeting::helo(argument).map(Command::Helo),
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

/// Выбирает RFC 5321 либо расширенный RFC 6531 предел до разбора команды.
pub fn command_line_limit(line: &[u8]) -> usize {
    let is_extended_mail = line
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"MAIL "))
        && std::str::from_utf8(line).is_ok_and(|text| {
            text.split_ascii_whitespace()
                .any(|parameter| parameter.eq_ignore_ascii_case("SMTPUTF8"))
        });
    match is_extended_mail {
        true => MAX_SMTPUTF8_MAIL_LINE_SIZE,
        _ => MAX_COMMAND_LINE_SIZE,
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
    let (reverse_path, suffix) = parse_path(argument, "FROM:", PathKind::Reverse)?;
    let parsed_parameters = match suffix.is_empty() {
        true => Vec::new(),
        false => suffix
            .split(' ')
            .map(parse_mail_parameter)
            .collect::<Result<Vec<_>, _>>()?,
    };
    let mut declared_size = None;
    let mut body_mode = BodyMode::SevenBit;
    let mut smtp_utf8 = false;
    let mut size_seen = false;
    let mut body_seen = false;
    let mut smtp_utf8_seen = false;
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
            MailParameter::SmtpUtf8 if !smtp_utf8_seen => {
                smtp_utf8 = true;
                smtp_utf8_seen = true;
            }
            MailParameter::Size(_) | MailParameter::Body(_) | MailParameter::SmtpUtf8 => {
                return Err(ParseError::Syntax);
            }
        }
    }
    if address::requires_smtp_utf8(&reverse_path) && !smtp_utf8 {
        return Err(ParseError::Syntax);
    }
    Ok(MailFrom {
        reverse_path,
        declared_size,
        body_mode,
        smtp_utf8,
    })
}

/// MAIL не рекламирует иные расширения, а RCPT пока принимает только path.
fn parse_recipient(argument: &str) -> Result<String, ParseError> {
    let (recipient, suffix) = parse_path(argument, "TO:", PathKind::Forward)?;
    match suffix.is_empty() {
        true => Ok(recipient),
        false => Err(ParseError::UnsupportedParameter),
    }
}

/// Выделяет angle path и оставляет хвост ESMTP параметров для typed parser-а.
fn parse_path<'a>(
    argument: &'a str,
    prefix: &str,
    kind: PathKind,
) -> Result<(String, &'a str), ParseError> {
    let (actual, rest) = argument.split_once(':').ok_or(ParseError::Syntax)?;
    let expected = prefix.strip_suffix(':').unwrap_or(prefix);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(ParseError::Syntax);
    }
    // RFC 5321 требует path сразу после двоеточия: пробел здесь не входит
    // в production MAIL/RCPT и не должен нормализоваться перед разбором.
    let value = rest;
    let Some(path) = value.strip_prefix('<') else {
        return Err(ParseError::Syntax);
    };
    let Some(terminator) = closing_path_angle(path) else {
        return Err(ParseError::Syntax);
    };
    let (path_content, remainder) = path.split_at(terminator);
    let Some(suffix) = remainder.strip_prefix('>') else {
        return Err(ParseError::Syntax);
    };
    if path_content.len().saturating_add(2) > MAX_PATH_SIZE {
        return Err(ParseError::Syntax);
    }
    let mailbox = address::without_source_route(path_content).ok_or(ParseError::Syntax)?;
    match (kind, address::classify_mailbox(mailbox)) {
        (PathKind::Reverse, address::MailboxForm::NullReversePath)
        | (_, address::MailboxForm::DotAtom)
        | (_, address::MailboxForm::QuotedLocalPart)
        | (_, address::MailboxForm::AddressLiteral)
        | (PathKind::Forward, address::MailboxForm::BarePostmaster) => {}
        _ => return Err(ParseError::Syntax),
    }
    match suffix.is_empty() {
        true => Ok((mailbox.to_owned(), "")),
        false => parameter_suffix(suffix).map(|parameters| (mailbox.to_owned(), parameters)),
    }
}

/// Проверяет разделители ESMTP-параметров: grammar допускает только одиночный SP.
fn parameter_suffix(suffix: &str) -> Result<&str, ParseError> {
    let Some(parameters) = suffix.strip_prefix(' ') else {
        return Err(ParseError::Syntax);
    };
    if parameters.is_empty() {
        return Err(ParseError::Syntax);
    }
    if parameters.contains('\t') {
        return Err(ParseError::Syntax);
    }
    if parameters.split(' ').any(str::is_empty) {
        return Err(ParseError::Syntax);
    }
    Ok(parameters)
}

/// Находит завершающий `>` вне quoted local-part и его escaped символов.
fn closing_path_angle(path: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    for (offset, character) in path.char_indices() {
        match (quoted, escaped, character) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (true, false, '"') => quoted = false,
            (false, _, '"') => quoted = true,
            (false, _, '>') => return Some(offset),
            _ => {}
        }
    }
    None
}

/// Роль SMTP path задаёт допустимые специальные mailbox формы.
#[derive(Clone, Copy)]
enum PathKind {
    Reverse,
    Forward,
}

/// Преобразует ESMTP parameter keyword в явно поддерживаемый enum.
fn parse_mail_parameter(parameter: &str) -> Result<MailParameter, ParseError> {
    let (name, value) = match parameter.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (parameter, None),
    };
    match (name.to_ascii_uppercase().as_str(), value) {
        ("SIZE", Some(value)) => parse_size(value).map(MailParameter::Size),
        ("BODY", Some(value)) => parse_body_mode(value).map(MailParameter::Body),
        ("SMTPUTF8", None) => Ok(MailParameter::SmtpUtf8),
        ("SIZE" | "BODY" | "SMTPUTF8", _) => Err(ParseError::Syntax),
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
    SmtpUtf8,
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

#[cfg(test)]
#[path = "path_whitespace_tests.rs"]
mod path_whitespace_tests;
