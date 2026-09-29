//! Семантический разбор поддерживаемого SCRAM-SHA-1 client-first профиля.
use super::scram_attributes::{self, AttributeName, ParseError};

/// GS2 и SASLprep обработанные поля первого SCRAM client message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientFirst {
    pub username: String,
    pub client_first_bare: String,
    nonce_range: std::ops::Range<usize>,
    pub gs2_header: Gs2Header,
}

impl ClientFirst {
    /// Возвращает nonce как срез общего client-first transcript без копирования.
    pub fn nonce(&self) -> &str {
        self.client_first_bare
            .get(self.nonce_range.clone())
            .expect("nonce range is validated while parsing client-first")
    }
}

/// Поддерживаемые GS2 header варианты после строгого разбора client-first.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gs2Header {
    NoChannelBinding,
    TlsExporter,
}

impl Gs2Header {
    /// Возвращает wire octets, входящие в SCRAM `c=` binding.
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::NoChannelBinding => b"n,,",
            Self::TlsExporter => b"p=tls-exporter,,",
        }
    }
}

/// Профиль намеренно принимает только `n,,`: без authzid и channel binding.
pub fn parse(message: &str) -> Result<ClientFirst, ClientFirstError> {
    let bare = message
        .strip_prefix("n,,")
        .ok_or(ClientFirstError::UnsupportedGs2Header)?;
    parse_bare(bare.to_owned(), Gs2Header::NoChannelBinding)
}

/// Принимает во владение уже декодированное сообщение без копии большого nonce.
pub fn parse_owned(message: String) -> Result<ClientFirst, ClientFirstError> {
    parse_owned_with_prefix(message, "n,,", Gs2Header::NoChannelBinding)
}

/// Принимает только RFC 7677 PLUS GS2 header без authzid.
pub fn parse_tls_exporter(message: &str) -> Result<ClientFirst, ClientFirstError> {
    let bare = message
        .strip_prefix("p=tls-exporter,,")
        .ok_or(ClientFirstError::UnsupportedGs2Header)?;
    parse_bare(bare.to_owned(), Gs2Header::TlsExporter)
}

/// Принимает во владение PLUS transcript после проверки точного GS2 header.
pub fn parse_tls_exporter_owned(message: String) -> Result<ClientFirst, ClientFirstError> {
    parse_owned_with_prefix(message, "p=tls-exporter,,", Gs2Header::TlsExporter)
}

/// Удаляет только известный GS2 prefix на месте и передаёт владение bare полем.
fn parse_owned_with_prefix(
    mut message: String,
    prefix: &str,
    gs2_header: Gs2Header,
) -> Result<ClientFirst, ClientFirstError> {
    if !message.starts_with(prefix) {
        return Err(ClientFirstError::UnsupportedGs2Header);
    }
    message.drain(..prefix.len());
    parse_bare(message, gs2_header)
}

fn parse_bare(bare: String, gs2_header: Gs2Header) -> Result<ClientFirst, ClientFirstError> {
    let attributes = scram_attributes::parse(&bare).map_err(ClientFirstError::Attributes)?;
    let fields = match attributes.as_slice() {
        [username, nonce, extensions @ ..]
            if username.name == AttributeName::Username
                && nonce.name == AttributeName::Nonce
                && extensions
                    .iter()
                    .all(|extension| matches!(extension.name, AttributeName::Extension(_))) =>
        {
            (username.value, nonce.value)
        }
        _ => return Err(ClientFirstError::InvalidFieldSequence),
    };
    let username = unescape_username(fields.0)?;
    let prepared =
        crate::sasl::saslprep::prepare(&username, crate::sasl::saslprep::SaslPrepContext::Query)
            .map_err(|_| ClientFirstError::InvalidUsername)?;
    validate_nonce(fields.1)?;
    let nonce_start = fields.1.as_ptr() as usize - bare.as_ptr() as usize;
    let nonce_end = nonce_start + fields.1.len();
    drop(attributes);
    Ok(ClientFirst {
        username: prepared,
        client_first_bare: bare,
        nonce_range: nonce_start..nonce_end,
        gs2_header,
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
