//! Ограниченный синтаксический разбор атрибутов SCRAM RFC 5802.

/// Ограничивает SCRAM transcript, оставаясь ниже POP3 Base64 line budget.
pub const MAX_MESSAGE_SIZE: usize = 256 * 1024;
/// Client-final включает server nonce и channel binding сверх принятого client-first.
pub const MAX_CLIENT_FINAL_SIZE: usize = MAX_MESSAGE_SIZE + 136;
/// Максимальное число атрибутов в одном SCRAM сообщении.
pub const MAX_ATTRIBUTE_COUNT: usize = 26;

/// Закрытый набор имен атрибутов SCRAM, нужных SHA-1 профилю.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeName {
    MandatoryExtension,
    Username,
    Nonce,
    ChannelBinding,
    Proof,
    Salt,
    Iterations,
    ServerSignature,
    Error,
    Extension(char),
}

/// Одно уникальное имя и opaque значение из SCRAM attribute list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    pub name: AttributeName,
    pub value: String,
}

/// Причина отказа парсера без включения пользовательских данных в ошибку.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    EmptyMessage,
    MessageTooLarge,
    TooManyAttributes,
    InvalidAttribute,
    DuplicateAttribute,
    MandatoryExtensionUnsupported,
}

/// Разбирает список `name=value`, отвергая повторы и неизвестные обязательные расширения.
pub fn parse(message: &str) -> Result<Vec<Attribute>, ParseError> {
    parse_with_limit(message, MAX_MESSAGE_SIZE)
}

/// Разбирает client-final с запасом для добавленного server nonce и TLS binding.
pub fn parse_client_final(message: &str) -> Result<Vec<Attribute>, ParseError> {
    parse_with_limit(message, MAX_CLIENT_FINAL_SIZE)
}

/// Разбирает атрибуты до переданного профильного предела, не меняя grammar.
fn parse_with_limit(message: &str, limit: usize) -> Result<Vec<Attribute>, ParseError> {
    if message.is_empty() {
        return Err(ParseError::EmptyMessage);
    }
    if message.len() > limit {
        return Err(ParseError::MessageTooLarge);
    }
    if message.split(',').take(MAX_ATTRIBUTE_COUNT + 1).count() > MAX_ATTRIBUTE_COUNT {
        return Err(ParseError::TooManyAttributes);
    }
    let mut attributes = Vec::new();
    for part in message.split(',') {
        if attributes.len() == MAX_ATTRIBUTE_COUNT {
            return Err(ParseError::TooManyAttributes);
        }
        let (name, value) = part.split_once('=').ok_or(ParseError::InvalidAttribute)?;
        let mut characters = name.chars();
        let Some(name) = characters.next() else {
            return Err(ParseError::InvalidAttribute);
        };
        if characters.next().is_some()
            || !name.is_ascii_alphabetic()
            || value.is_empty()
            || !valid_value(value)
        {
            return Err(ParseError::InvalidAttribute);
        }
        let name = attribute_name(name);
        if name == AttributeName::MandatoryExtension {
            return Err(ParseError::MandatoryExtensionUnsupported);
        }
        if attributes
            .iter()
            .any(|attribute: &Attribute| attribute.name == name)
        {
            return Err(ParseError::DuplicateAttribute);
        }
        attributes.push(Attribute {
            name,
            value: value.to_owned(),
        });
    }
    Ok(attributes)
}

/// Сопоставляет один wire-символ закрытому типу атрибута.
fn attribute_name(value: char) -> AttributeName {
    match value {
        'm' => AttributeName::MandatoryExtension,
        'n' => AttributeName::Username,
        'r' => AttributeName::Nonce,
        'c' => AttributeName::ChannelBinding,
        'p' => AttributeName::Proof,
        's' => AttributeName::Salt,
        'i' => AttributeName::Iterations,
        'v' => AttributeName::ServerSignature,
        'e' => AttributeName::Error,
        other => AttributeName::Extension(other),
    }
}

/// Ограничивает значение допустимыми SCRAM value-char после UTF-8 декодирования.
fn valid_value(value: &str) -> bool {
    value
        .chars()
        .all(|character| character != '\0' && character != ',')
}

#[cfg(test)]
#[path = "scram_attributes_tests.rs"]
mod tests;
