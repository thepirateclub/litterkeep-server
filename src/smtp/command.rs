//! Типизированная грамматика SMTP команд и расширенного MAIL FROM.

#[path = "address.rs"]
mod address;
#[path = "address_literal.rs"]
mod address_literal;
#[path = "auth_command.rs"]
mod auth_command;
#[path = "command_error.rs"]
mod command_error;
#[path = "greeting.rs"]
mod greeting;
#[path = "mail_path.rs"]
mod mail_path;
#[path = "verify_command.rs"]
mod verify_command;
#[cfg(test)]
#[path = "verify_command_tests.rs"]
mod verify_command_tests;
pub(crate) use address::canonical_parts;
#[cfg(target_os = "linux")]
pub(crate) use address::requires_smtp_utf8;
pub use verify_command::{ReplyEncoding, VerifyRequest};

/// Команда SMTP после разбора wire-строки без потери ESMTP параметров.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Ehlo(String),
    Helo(String),
    Mail(MailFrom),
    Rcpt(String),
    Data,
    /// SASL PLAIN с optional initial response в base64-представлении.
    AuthPlain(Option<String>),
    /// Проверка имени, не раскрывающая наличие локального пользователя.
    Vrfy(VerifyRequest),
    /// Запрос списка участников, который локальная политика запрещает.
    Expn(VerifyRequest),
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
pub use command_error::ParseError;

/// Локальный предел совпадает с числом, передаваемым в EHLO SIZE.
pub const MAX_MESSAGE_SIZE: u64 = 25 * 1024 * 1024;
/// Максимальная DATA-строка после dot-unstuffing вместе с завершающим CRLF.
pub const MAX_DATA_LINE_SIZE: usize = 1000;
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
    let (verb, argument) = command_parts(line);
    match verb.to_ascii_uppercase().as_str() {
        "EHLO" => greeting::ehlo(argument).map(Command::Ehlo),
        "HELO" => greeting::helo(argument).map(Command::Helo),
        "MAIL" => mail_path::parse_mail_from(argument).map(Command::Mail),
        "RCPT" => mail_path::parse_recipient(argument).map(Command::Rcpt),
        "DATA" => match argument.is_empty() {
            true => Ok(Command::Data),
            false => Err(ParseError::Syntax),
        },
        "AUTH" => auth_command::parse(argument),
        "VRFY" => verify_command::parse(argument).map(Command::Vrfy),
        "EXPN" => verify_command::parse(argument).map(Command::Expn),
        "HELP" => parse_optional_string(argument).map(Command::Help),
        "STARTTLS" => match argument.is_empty() {
            true => Ok(Command::StartTls),
            false => Err(ParseError::Syntax),
        },
        "NOOP" => parse_optional_string(argument).map(Command::Noop),
        "RSET" => match argument.is_empty() {
            true => Ok(Command::Rset),
            false => Err(ParseError::Syntax),
        },
        "QUIT" => match argument.is_empty() {
            true => Ok(Command::Quit),
            false => Err(ParseError::Syntax),
        },
        _ => Err(ParseError::UnrecognizedCommand),
    }
}

/// Отделяет verb только по RFC-пробелу, сохраняя TAB как неверный аргумент.
fn command_parts(line: &str) -> (&str, &str) {
    match line.split_once(' ') {
        Some((verb, argument)) => (verb, argument),
        None => match line.split_once('\t') {
            Some((verb, _)) => (verb, "\t"),
            None => (line, ""),
        },
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
    // RFC 5321 String ограничен printable ASCII; удалять можно только разделители SP.
    let value = argument.trim_matches(' ');
    let valid = !value.is_empty() && value.bytes().all(|byte| (32..=126).contains(&byte));
    match valid {
        true => Ok(value.to_owned()),
        false => Err(ParseError::Syntax),
    }
}

/// Разбирает необязательный параметр HELP/NOOP без изменения его значения.
fn parse_optional_string(argument: &str) -> Result<Option<String>, ParseError> {
    match argument.trim_matches(' ').is_empty() {
        true => Ok(None),
        false => parse_string_command(argument).map(Some),
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "path_whitespace_tests.rs"]
mod path_whitespace_tests;

#[cfg(test)]
#[path = "ascii_string_wsp_tests.rs"]
mod ascii_string_wsp_tests;
