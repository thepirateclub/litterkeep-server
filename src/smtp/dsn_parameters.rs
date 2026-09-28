//! Типизированные параметры расширения SMTP DSN из RFC 3461.

use super::ParseError;
use std::collections::HashSet;

#[path = "dsn_utf8.rs"]
mod utf8;

/// Правило отправки итогового отчёта для одного получателя.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NotifyCondition {
    /// Запрашивает уведомление об успешной доставке.
    Success,
    /// Запрашивает уведомление о невозможности доставки.
    Failure,
    /// Разрешает уведомление о длительной задержке.
    Delay,
}

/// Список условий либо явный запрет всех уведомлений.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotifyPolicy {
    /// Для получателя не следует создавать DSN ни при каком результате.
    Never,
    /// Условия перечислены sender-ом; порядок исходных токенов не значим.
    Conditions(HashSet<NotifyCondition>),
}

/// Поддерживаемый RFC 3464 address-type для ORCPT.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalAddressType {
    /// Адрес представлен в формате RFC 822 mailbox.
    Rfc822,
    /// Адрес UTF-8, восстановленный из RFC 6533 unitext/xtext формы.
    Utf8,
}

/// Адрес, первоначально указанный SMTP sender-ом.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalRecipient {
    /// Зарегистрированный тип исходного адреса.
    pub address_type: OriginalAddressType,
    /// Декодированный адрес в исходном регистре; UTF-8 тип содержит UTF-8 octets.
    pub address: Vec<u8>,
}

/// Настройки RFC 3461 для одной команды RCPT TO.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecipientDsn {
    /// Отсутствие параметра сохраняет стандартную failure policy SMTP.
    pub notify: Option<NotifyPolicy>,
    /// Необязательный первоначальный адрес получателя.
    pub original_recipient: Option<OriginalRecipient>,
}

/// Настройки возврата содержимого и корреляции MAIL transaction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MailDsn {
    /// Полное исходное сообщение или только его заголовки в failure report.
    pub return_content: Option<ReturnContent>,
    /// Xtext-декодированный непрозрачный идентификатор sender-а.
    pub envelope_id: Option<Vec<u8>>,
}

/// Объём исходного сообщения, допустимый к возврату в DSN.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnContent {
    /// Возвращается только заголовочная часть.
    Headers,
    /// Возвращается полное исходное сообщение.
    Full,
}

/// Разбирает одно имя и значение RFC 3461 без сетевой политики поддержки.
pub(super) fn parse_notify(value: &str) -> Result<NotifyPolicy, ParseError> {
    if value.eq_ignore_ascii_case("NEVER") {
        return Ok(NotifyPolicy::Never);
    }
    let mut conditions = HashSet::new();
    for item in value.split(',') {
        let condition = match item.to_ascii_uppercase().as_str() {
            "SUCCESS" => NotifyCondition::Success,
            "FAILURE" => NotifyCondition::Failure,
            "DELAY" => NotifyCondition::Delay,
            _ => return Err(ParseError::Syntax),
        };
        if !conditions.insert(condition) {
            return Err(ParseError::Syntax);
        }
    }
    match conditions.is_empty() {
        true => Err(ParseError::Syntax),
        false => Ok(NotifyPolicy::Conditions(conditions)),
    }
}

/// Разбирает RFC 3461 xtext и требует printable US-ASCII до кодирования.
pub(super) fn decode_xtext(value: &str, maximum: usize) -> Result<Vec<u8>, ParseError> {
    if value.is_empty() || value.len() > maximum {
        return Err(ParseError::Syntax);
    }
    let mut decoded = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => decode_escape(&mut bytes, &mut decoded)?,
            b'!'..=b'~' if byte != b'=' => decoded.push(byte),
            _ => return Err(ParseError::Syntax),
        }
    }
    let printable = decoded.iter().all(|byte| (b' '..=b'~').contains(byte));
    match printable {
        true => Ok(decoded),
        false => Err(ParseError::Syntax),
    }
}

/// Декодирует только 7-bit-safe RFC 6533 utf-8-addr-xtext.
pub(super) fn decode_utf8_xtext(value: &str, maximum: usize) -> Result<Vec<u8>, ParseError> {
    utf8::decode(value, maximum)
}

/// Декодирует одну обязательную пару заглавных шестнадцатеричных цифр.
fn decode_escape(
    bytes: &mut impl Iterator<Item = u8>,
    decoded: &mut Vec<u8>,
) -> Result<(), ParseError> {
    let high = bytes
        .next()
        .and_then(upper_hex_digit)
        .ok_or(ParseError::Syntax)?;
    let low = bytes
        .next()
        .and_then(upper_hex_digit)
        .ok_or(ParseError::Syntax)?;
    decoded.push((high << 4) | low);
    Ok(())
}

/// Возвращает значение только для цифры или заглавной шестнадцатеричной буквы.
fn upper_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "dsn_parameter_tests.rs"]
mod tests;
