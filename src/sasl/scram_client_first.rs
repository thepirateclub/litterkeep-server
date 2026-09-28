//! Семантический разбор поддерживаемого SCRAM-SHA-1 client-first профиля.
use super::scram_attributes::{self, AttributeName, ParseError};

/// GS2 и SASLprep обработанные поля первого SCRAM client message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFirst {
    pub username: String,
    pub nonce: String,
    pub client_first_bare: String,
}

/// Профиль намеренно принимает только `n,,`: без authzid и channel binding.
pub fn parse(message: &str) -> Result<ClientFirst, ClientFirstError> {
    let bare = message
        .strip_prefix("n,,")
        .ok_or(ClientFirstError::UnsupportedGs2Header)?;
    let attributes = scram_attributes::parse(bare).map_err(ClientFirstError::Attributes)?;
    let fields = match attributes.as_slice() {
        [username, nonce, extensions @ ..]
            if username.name == AttributeName::Username
                && nonce.name == AttributeName::Nonce
                && extensions
                    .iter()
                    .all(|extension| matches!(extension.name, AttributeName::Extension(_))) =>
        {
            (username.value.as_str(), nonce.value.as_str())
        }
        _ => return Err(ClientFirstError::InvalidFieldSequence),
    };
    let username = unescape_username(fields.0)?;
    let prepared =
        crate::sasl::saslprep::prepare(&username, crate::sasl::saslprep::SaslPrepContext::Query)
            .map_err(|_| ClientFirstError::InvalidUsername)?;
    validate_nonce(fields.1)?;
    Ok(ClientFirst {
        username: prepared,
        nonce: fields.1.to_owned(),
        client_first_bare: bare.to_owned(),
    })
}

/// Причина отказа без копирования недоверенного значения в wire-ответ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientFirstError {
    UnsupportedGs2Header,
    Attributes(ParseError),
    InvalidFieldSequence,
    InvalidUsername,
    InvalidNonce,
}

/// Декодирует RFC 5802 saslname escaping без нестрогой замены последовательностей.
fn unescape_username(value: &str) -> Result<String, ClientFirstError> {
    if value.is_empty() {
        return Err(ClientFirstError::InvalidUsername);
    }
    let mut characters = value.chars().peekable();
    let mut decoded = String::with_capacity(value.len());
    while let Some(character) = characters.next() {
        match character {
            '=' => match (characters.next(), characters.next()) {
                (Some('2'), Some('C')) => decoded.push(','),
                (Some('3'), Some('D')) => decoded.push('='),
                _ => return Err(ClientFirstError::InvalidUsername),
            },
            value => decoded.push(value),
        }
    }
    Ok(decoded)
}

/// SCRAM nonce непустой и не допускает comma-разделитель атрибутов.
fn validate_nonce(value: &str) -> Result<(), ClientFirstError> {
    match value.is_empty()
        || value
            .bytes()
            .any(|octet| !octet.is_ascii_graphic() || octet == b',')
    {
        true => Err(ClientFirstError::InvalidNonce),
        false => Ok(()),
    }
}

#[cfg(test)]
#[path = "scram_client_first_tests.rs"]
mod tests;
