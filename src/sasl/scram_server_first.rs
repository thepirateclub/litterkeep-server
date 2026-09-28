//! Создание SCRAM-SHA-1 server-first challenge по RFC 5802.
use super::{scram_client_first::ClientFirst, scram_sha1_record::ScramSha1Record};
use std::io;

/// Размер случайного server nonce до Base64 кодирования.
const SERVER_NONCE_SIZE: usize = 18;

/// Подготовленный challenge и объединённый nonce для проверки client-final.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerFirst {
    pub nonce: String,
    pub message: String,
}

impl ServerFirst {
    /// Создаёт challenge с непредсказуемым suffix через provider Rustls.
    pub fn create(client: &ClientFirst, credential: &ScramSha1Record) -> io::Result<Self> {
        let random = rustls::crypto::ring::default_provider().secure_random;
        let mut entropy = [0_u8; SERVER_NONCE_SIZE];
        random
            .fill(&mut entropy)
            .map_err(|_| io::Error::other("SCRAM nonce generation failed"))?;
        let suffix = crate::sasl::encode_base64(&entropy);
        Self::with_suffix(client, credential, &suffix)
    }

    /// Строит challenge для проверяемого случайного suffix, доступного тестам.
    fn with_suffix(
        client: &ClientFirst,
        credential: &ScramSha1Record,
        suffix: &str,
    ) -> io::Result<Self> {
        validate_nonce_suffix(suffix)?;
        let nonce = format!("{}{suffix}", client.nonce);
        let salt = crate::sasl::encode_base64(credential.salt());
        let message = format!("r={nonce},s={salt},i={}", credential.iterations());
        Ok(Self { nonce, message })
    }
}

/// Ограничивает suffix тем же printable ASCII профилем, что и SCRAM nonce.
fn validate_nonce_suffix(value: &str) -> io::Result<()> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|octet| octet.is_ascii_graphic() && octet != b',');
    match valid {
        true => Ok(()),
        false => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid SCRAM server nonce suffix",
        )),
    }
}

#[cfg(test)]
#[path = "scram_server_first_tests.rs"]
mod tests;
