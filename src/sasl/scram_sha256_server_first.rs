//! Формирует server-first challenge для SCRAM-SHA-256.

use super::{scram_client_first::ClientFirst, scram_sha256_record::ScramSha256Record};
use std::io;

/// Размер случайного server nonce до Base64 кодирования.
const SERVER_NONCE_SIZE: usize = 18;

/// Challenge и полный nonce, необходимые для расчёта transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerFirst {
    pub nonce: String,
    pub message: String,
}

/// Добавляет криптографически случайный suffix к client nonce.
pub fn create(client: &ClientFirst, credential: &ScramSha256Record) -> io::Result<ServerFirst> {
    let random = rustls::crypto::ring::default_provider().secure_random;
    let mut entropy = [0_u8; SERVER_NONCE_SIZE];
    random
        .fill(&mut entropy)
        .map_err(|_| io::Error::other("SCRAM nonce generation failed"))?;
    let suffix = crate::sasl::encode_base64(&entropy);
    let nonce = format!("{}{suffix}", client.nonce());
    let salt = crate::sasl::encode_base64(credential.salt());
    let message = format!("r={nonce},s={salt},i={}", credential.iterations());
    Ok(ServerFirst { nonce, message })
}
