//! Типизированная грамматика аргументов SMTP VRFY и EXPN.

use super::ParseError;

/// Возможности клиента по приёму UTF-8 в ответе VRFY/EXPN.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplyEncoding {
    /// Ответ клиента должен оставаться в ASCII.
    AsciiOnly,
    /// RFC 6531 разрешает UTF-8 в ответе этой команды.
    Utf8Allowed,
}

/// Проверенная строка команды и режим кодирования ответа клиента.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyRequest {
    /// Непрозрачная строка идентификации для policy-слоя.
    pub query: String,
    /// Объявленная клиентом возможность читать UTF-8 ответ.
    pub reply_encoding: ReplyEncoding,
}

/// Разбирает общую VRFY/EXPN форму RFC 6531 без создания побочных эффектов.
pub(super) fn parse(argument: &str) -> Result<VerifyRequest, ParseError> {
    match argument.rsplit_once(' ') {
        Some((query, parameter)) => match parameter_kind(parameter) {
            ParameterKind::SmtpUtf8 => request(query, ReplyEncoding::Utf8Allowed),
            ParameterKind::InvalidSmtpUtf8 => Err(ParseError::Syntax),
            ParameterKind::Other => request(argument, ReplyEncoding::AsciiOnly),
        },
        None => request(argument, ReplyEncoding::AsciiOnly),
    }
}

/// Отличает валидный valueless extension token от неизвестного/неверного.
fn parameter_kind(parameter: &str) -> ParameterKind {
    match parameter.eq_ignore_ascii_case("SMTPUTF8") {
        true => ParameterKind::SmtpUtf8,
        false => match parameter.split_once('=') {
            Some((name, _)) if name.eq_ignore_ascii_case("SMTPUTF8") => {
                ParameterKind::InvalidSmtpUtf8
            }
            _ => ParameterKind::Other,
        },
    }
}

/// Проверяет непустую SMTP String и сохраняет её UTF-8 без Unicode trim.
fn request(query: &str, reply_encoding: ReplyEncoding) -> Result<VerifyRequest, ParseError> {
    match valid_string(query) {
        true => Ok(VerifyRequest {
            query: query.to_owned(),
            reply_encoding,
        }),
        false => Err(ParseError::Syntax),
    }
}

/// Принимает printable ASCII и корректные UTF-8 scalar values без controls.
fn valid_string(query: &str) -> bool {
    match (
        query.is_empty(),
        query.starts_with(' '),
        query.ends_with(' '),
    ) {
        (false, false, false) => query.chars().all(valid_character),
        _ => false,
    }
}

/// RFC String не пропускает управляющие ASCII/Unicode scalar values.
fn valid_character(character: char) -> bool {
    match character {
        ' '..='~' => true,
        _ => valid_utf8_scalar(character),
    }
}

/// Проверяет единый UTF-8 scalar без пропуска Unicode control characters.
fn valid_utf8_scalar(character: char) -> bool {
    matches!(
        (character.is_ascii(), character.is_control()),
        (false, false)
    )
}

/// Класс синтаксиса последнего аргумента после SMTP SP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParameterKind {
    SmtpUtf8,
    InvalidSmtpUtf8,
    Other,
}
