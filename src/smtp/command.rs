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
    StartTls,
    Noop,
    Rset,
    Quit,
}

/// Типизированные поля расширенной команды MAIL FROM RFC 1870.
#[derive(Debug, Eq, PartialEq)]
pub struct MailFrom {
    pub reverse_path: String,
    pub declared_size: Option<u128>,
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
        "STARTTLS" if argument.is_empty() => Ok(Command::StartTls),
        "NOOP" if argument.is_empty() => Ok(Command::Noop),
        "RSET" if argument.is_empty() => Ok(Command::Rset),
        "QUIT" if argument.is_empty() => Ok(Command::Quit),
        _ => Err(ParseError::Syntax),
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
        .map(parse_size_parameter)
        .collect::<Result<Vec<_>, _>>()?;
    let declared_size = match parsed_parameters.as_slice() {
        [] => None,
        [size] => Some(*size),
        _ => return Err(ParseError::Syntax),
    };
    Ok(MailFrom {
        reverse_path,
        declared_size,
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
fn parse_size_parameter(parameter: &str) -> Result<u128, ParseError> {
    let (name, value) = parameter.split_once('=').ok_or(ParseError::Syntax)?;
    match name.to_ascii_uppercase().as_str() {
        "SIZE" => parse_size(value),
        _ => Err(ParseError::UnsupportedParameter),
    }
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
            }))
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
            (
                "MAIL FROM:<alice@test> BODY=8BITMIME\r\n",
                ParseError::UnsupportedParameter,
            ),
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
        assert_eq!(parse("NOOP\r\n"), Ok(Command::Noop));
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
