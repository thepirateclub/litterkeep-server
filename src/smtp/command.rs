//! Типизированная грамматика SMTP команд и расширенного MAIL FROM.

#[path = "address.rs"]
mod address;
#[path = "address_literal.rs"]
mod address_literal;
#[path = "auth_command.rs"]
mod auth_command;
#[path = "bdat_command.rs"]
mod bdat_command;
#[path = "command_error.rs"]
mod command_error;
#[path = "dsn_parameters.rs"]
mod dsn_parameters;
#[path = "greeting.rs"]
mod greeting;
#[cfg(test)]
#[path = "mail_line_limit_tests.rs"]
mod mail_line_limit_tests;
#[path = "mail_path.rs"]
mod mail_path;
#[path = "verify_command.rs"]
mod verify_command;
#[cfg(test)]
#[path = "verify_command_tests.rs"]
mod verify_command_tests;
pub(crate) use address::canonical_parts;
pub(crate) use address::requires_smtp_utf8;
pub use bdat_command::BdatCommand;
pub use dsn_parameters::{
    MailDsn, NotifyCondition, NotifyPolicy, OriginalAddressType, OriginalRecipient, RecipientDsn,
    ReturnContent,
};
pub use verify_command::{ReplyEncoding, VerifyRequest};

/// Команда SMTP после разбора wire-строки без потери ESMTP параметров.
#[derive(Debug, Eq, PartialEq)]
pub enum Command {
    Ehlo(String),
    Helo(String),
    Mail(MailFrom),
    Rcpt(String),
    /// MAIL с корректно разобранными, но пока не поддерживаемыми параметрами DSN.
    MailWithDsn(MailFrom, MailDsn),
    /// RCPT с корректно разобранными, но пока не поддерживаемыми параметрами DSN.
    RcptWithDsn(RcptTo),
    Data,
    /// Один octet-counted chunk согласно RFC 3030.
    Bdat(BdatCommand),
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

/// Адрес и RFC 3461 настройки одной команды RCPT TO.
#[derive(Debug, Eq, PartialEq)]
pub struct RcptTo {
    /// Проверенный SMTP forward-path без угловых скобок.
    pub address: String,
    /// Типизированные запросы sender-а для будущих отчётов.
    pub dsn: RecipientDsn,
}

/// RFC 6152/3030 body profile; unsupported modes are rejected before MAIL.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyMode {
    SevenBit,
    EightBitMime,
    /// Значение RFC 3030 распознаётся, но сервер пока не объявляет поддержку.
    BinaryMime,
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
/// RFC 3030 расширяет MAIL ещё на шестнадцать октетов для BINARYMIME.
pub const MAX_BINARYMIME_MAIL_LINE_SIZE: usize = MAX_COMMAND_LINE_SIZE + 16;
/// Совместная длина расширений RFC 6531 и RFC 3030 складывается.
pub const MAX_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE: usize = MAX_COMMAND_LINE_SIZE + 10 + 16;
/// RFC 1870 разрешает 26 дополнительных октетов для параметра SIZE.
pub const MAX_SIZE_MAIL_LINE_SIZE: usize = MAX_COMMAND_LINE_SIZE + 26;
/// SIZE может сочетаться с увеличением пути SMTPUTF8 и BODY=BINARYMIME.
pub const MAX_SIZE_SMTPUTF8_MAIL_LINE_SIZE: usize = MAX_SIZE_MAIL_LINE_SIZE + 10;
/// SIZE и BINARYMIME складывают независимые расширения строки MAIL.
pub const MAX_SIZE_BINARYMIME_MAIL_LINE_SIZE: usize = MAX_SIZE_MAIL_LINE_SIZE + 16;
/// Максимум MAIL со всеми тремя расширениями командной строки.
pub const MAX_SIZE_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE: usize = MAX_SIZE_MAIL_LINE_SIZE + 10 + 16;
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
        "MAIL" => mail_path::parse_mail_from(argument).map(|(mail, dsn)| match dsn {
            Some(dsn) => Command::MailWithDsn(mail, dsn),
            None => Command::Mail(mail),
        }),
        "RCPT" => mail_path::parse_recipient(argument).map(|recipient| match recipient.dsn {
            dsn if dsn == RecipientDsn::default() => Command::Rcpt(recipient.address),
            _ => Command::RcptWithDsn(recipient),
        }),
        "BDAT" => bdat_command::parse(argument).map(Command::Bdat),
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
    let is_mail_command = line
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"MAIL "));
    match (is_mail_command, std::str::from_utf8(line)) {
        (true, Ok(text)) => mail_line_limit(text),
        _ => MAX_COMMAND_LINE_SIZE,
    }
}

/// Суммирует только allowances расширений, объявленных в MAIL-команде.
fn mail_line_limit(line: &str) -> usize {
    let extensions = line
        .split_ascii_whitespace()
        .filter_map(MailLineExtension::parse)
        .fold(MailLineExtensions::default(), MailLineExtensions::include);
    extensions.limit()
}

/// Расширение ESMTP, которое увеличивает допустимую длину команды MAIL.
#[derive(Clone, Copy)]
enum MailLineExtension {
    Size,
    SmtpUtf8,
    BinaryMime,
}

impl MailLineExtension {
    /// Классифицирует параметры MAIL без строковых сравнений verbatim.
    fn parse(token: &str) -> Option<Self> {
        let bytes = token.as_bytes();
        let size = bytes
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"SIZE="));
        let smtp_utf8 = bytes.eq_ignore_ascii_case(b"SMTPUTF8");
        let binary_mime = bytes.eq_ignore_ascii_case(b"BODY=BINARYMIME");
        match (size, smtp_utf8, binary_mime) {
            (true, _, _) => Some(Self::Size),
            (_, true, _) => Some(Self::SmtpUtf8),
            (_, _, true) => Some(Self::BinaryMime),
            (false, false, false) => None,
        }
    }
}

/// Набор независимых дополнений, суммируемых для RFC 5321 line limit.
#[derive(Default)]
struct MailLineExtensions {
    size: bool,
    smtp_utf8: bool,
    binary_mime: bool,
}

impl MailLineExtensions {
    /// Добавляет распознанное расширение без повторного увеличения лимита.
    fn include(mut self, extension: MailLineExtension) -> Self {
        match extension {
            MailLineExtension::Size => self.size = true,
            MailLineExtension::SmtpUtf8 => self.smtp_utf8 = true,
            MailLineExtension::BinaryMime => self.binary_mime = true,
        }
        self
    }

    /// Возвращает RFC-предел, включая CRLF, по набору параметров MAIL.
    fn limit(&self) -> usize {
        match (self.size, self.smtp_utf8, self.binary_mime) {
            (false, false, false) => MAX_COMMAND_LINE_SIZE,
            (false, false, true) => MAX_BINARYMIME_MAIL_LINE_SIZE,
            (false, true, false) => MAX_SMTPUTF8_MAIL_LINE_SIZE,
            (false, true, true) => MAX_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE,
            (true, false, false) => MAX_SIZE_MAIL_LINE_SIZE,
            (true, false, true) => MAX_SIZE_BINARYMIME_MAIL_LINE_SIZE,
            (true, true, false) => MAX_SIZE_SMTPUTF8_MAIL_LINE_SIZE,
            (true, true, true) => MAX_SIZE_SMTPUTF8_BINARYMIME_MAIL_LINE_SIZE,
        }
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
#[path = "dsn_command_tests.rs"]
mod dsn_command_tests;

#[cfg(test)]
#[path = "binarymime_tests.rs"]
mod binarymime_tests;

#[cfg(test)]
#[path = "path_whitespace_tests.rs"]
mod path_whitespace_tests;

#[cfg(test)]
#[path = "ascii_string_wsp_tests.rs"]
mod ascii_string_wsp_tests;
