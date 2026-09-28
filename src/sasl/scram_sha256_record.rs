//! Версионированный долговечный verifier record SCRAM-SHA-256.

use super::scram_sha256_verifier::{
    MAX_SALT_SIZE, MIN_SALT_SIZE, ScramSha256Verifier, valid_parameters,
};

const RECORD_VERSION: &str = "litterkeep-scram-sha256-v1";

/// Хранит salt/cost и SCRAM verifier без исходного пароля.
pub struct ScramSha256Record {
    salt: Vec<u8>,
    iterations: u32,
    verifier: ScramSha256Verifier,
}

impl ScramSha256Record {
    /// Создаёт credential из пароля с криптографически случайной солью.
    pub fn create(password: &[u8]) -> Option<Self> {
        let (salt, iterations, verifier) = ScramSha256Verifier::create(password)?;
        Some(Self {
            salt,
            iterations,
            verifier,
        })
    }

    /// Строго разбирает версию, поля, salt size, cost и обе длины ключей.
    pub fn parse(encoded: &str) -> Option<Self> {
        let mut fields = encoded.trim_end_matches('\n').split('$');
        let version = fields.next()?;
        let iterations = fields.next()?.parse::<u32>().ok()?;
        let salt = decode_hex(fields.next()?)?;
        let stored_key = decode_fixed_key(fields.next()?)?;
        let server_key = decode_fixed_key(fields.next()?)?;
        match (version, fields.next(), valid_parameters(&salt, iterations)) {
            (RECORD_VERSION, None, true) => Some(Self {
                salt,
                iterations,
                verifier: ScramSha256Verifier::from_keys(stored_key, server_key),
            }),
            _ => None,
        }
    }

    /// Сериализует запись с отдельным discriminator формата SHA-256.
    pub fn encode(&self) -> String {
        format!(
            "{RECORD_VERSION}${}${}${}${}\n",
            self.iterations,
            encode_hex(&self.salt),
            encode_hex(self.verifier.stored_key()),
            encode_hex(self.verifier.server_key())
        )
    }

    /// Возвращает salt для SCRAM server-first.
    pub fn salt(&self) -> &[u8] {
        &self.salt
    }

    /// Возвращает PBKDF2 iteration count для SCRAM server-first.
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Возвращает производный verifier для проверки клиентского proof.
    pub fn verifier(&self) -> &ScramSha256Verifier {
        &self.verifier
    }
}

/// Декодирует UTF-8 hex-представление без прямой индексации входа.
fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = pair.first().copied().and_then(hex_value)?;
            let low = pair.get(1).copied().and_then(hex_value)?;
            Some((high << 4) | low)
        })
        .collect()
}

/// Проверяет, что hex-поле кодирует ровно один 32-октетный ключ.
fn decode_fixed_key(value: &str) -> Option<[u8; 32]> {
    decode_hex(value)?.try_into().ok()
}

/// Декодирует один разрешённый ASCII hex digit.
fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Кодирует двоичные credential-поля в lowercase hex.
fn encode_hex(value: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len().saturating_mul(2));
    for byte in value {
        let high = usize::from(byte >> 4);
        let low = usize::from(byte & 15);
        output.push(char::from(DIGITS.get(high).copied().unwrap_or(b'0')));
        output.push(char::from(DIGITS.get(low).copied().unwrap_or(b'0')));
    }
    output
}

const _: () = assert!(MAX_SALT_SIZE >= MIN_SALT_SIZE);

#[cfg(test)]
#[path = "scram_sha256_record_tests.rs"]
mod tests;
