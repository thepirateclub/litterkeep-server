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
mod tests {
    use super::*;

    #[test]
    fn parses_size_parameter_as_typed_mail_metadata() {
        assert_eq!(
            parse("mail FROM:<alice@test> size=37\r\n"),
            Ok(Command::Mail(MailFrom {
                reverse_path: "alice@test".into(),
                declared_size: Some(37),
                body_mode: BodyMode::SevenBit,
            }))
        );
    }

    /// Разбирает RFC 6152 body modes и поддерживает совместный SIZE.
    #[test]
    fn parses_8bitmime_body_mode_and_parameter_order() {
        assert_eq!(
            parse("MAIL FROM:<alice@test> BODY=8BITMIME SIZE=23\r\n"),
            Ok(Command::Mail(MailFrom {
                reverse_path: "alice@test".into(),
                declared_size: Some(23),
                body_mode: BodyMode::EightBitMime,
            }))
        );
        assert_eq!(
            parse("MAIL FROM:<alice@test> BODY=7BIT\r\n"),
            Ok(Command::Mail(MailFrom {
                reverse_path: "alice@test".into(),
                declared_size: None,
                body_mode: BodyMode::SevenBit,
            }))
        );
        assert_eq!(
            parse("MAIL FROM:<alice@test> BODY=8BITMIME BODY=7BIT\r\n"),
            Err(ParseError::Syntax)
        );
    }

    /// Отклоняет некорректные значения, повторы и не рекламируемые параметры.
    #[test]
    fn distinguishes_invalid_size_from_unsupported_parameter() {
        let cases = [
            ("MAIL FROM:<alice@test> SIZE=\r\n", ParseError::Syntax),
            ("MAIL FROM:<alice@test> SIZE=+1\r\n", ParseError::Syntax),
            (
                "MAIL FROM:<alice@test> SIZE=123456789012345678901\r\n",
                ParseError::Syntax,
            ),
            (
                "MAIL FROM:<alice@test> SIZE=1 SIZE=2\r\n",
                ParseError::Syntax,
            ),
            ("MAIL FROM:<alice@test> BODY=FOO\r\n", ParseError::Syntax),
            (
                "RCPT TO:<bob@test> NOTIFY=SUCCESS\r\n",
                ParseError::UnsupportedParameter,
            ),
        ];
        for (wire, expected) in cases {
            assert_eq!(parse(wire), Err(expected), "wire command: {wire}");
        }
    }

    #[test]
    fn accepts_empty_reverse_path_but_requires_recipient_path() {
        assert_eq!(
            parse("MAIL FROM:<>\r\n"),
            Ok(Command::Mail(MailFrom {
                reverse_path: String::new(),
                declared_size: None,
                body_mode: BodyMode::SevenBit,
            }))
        );
        assert_eq!(
            parse("RCPT TO:<bob@test>\r\n"),
            Ok(Command::Rcpt("bob@test".into()))
        );
        assert_eq!(parse("RCPT TO:<>\r\n"), Err(ParseError::Syntax));
    }

    #[test]
    fn parses_supported_commands_and_auth_mechanism() {
        assert_eq!(
            parse("AUTH PLAIN AGJvYgB0ZXN0LXBhc3N3b3Jk\r\n"),
            Ok(Command::AuthPlain("AGJvYgB0ZXN0LXBhc3N3b3Jk".into()))
        );
        assert_eq!(parse("STARTTLS\r\n"), Ok(Command::StartTls));
        assert_eq!(parse("NOOP\r\n"), Ok(Command::Noop(None)));
        assert_eq!(
            parse("NOOP keep state\r\n"),
            Ok(Command::Noop(Some("keep state".into())))
        );
        assert_eq!(
            parse("VRFY alice@test\r\n"),
            Ok(Command::Vrfy("alice@test".into()))
        );
        assert_eq!(parse("EXPN team\r\n"), Ok(Command::Expn("team".into())));
        assert_eq!(parse("HELP\r\n"), Ok(Command::Help(None)));
        assert_eq!(
            parse("HELO sender.test\r\n"),
            Ok(Command::Helo("sender.test".into()))
        );
        assert_eq!(
            parse("AUTH LOGIN response\r\n"),
            Err(ParseError::UnsupportedAuthenticationMechanism)
        );
    }
}
