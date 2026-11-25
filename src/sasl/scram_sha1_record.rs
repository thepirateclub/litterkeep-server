//! Версионированное текстовое представление SCRAM-SHA-1 account verifier.
use super::scram_sha1_verifier::{
    MAX_ITERATIONS, MAX_SALT_SIZE, MIN_ITERATIONS, MIN_SALT_SIZE, ScramSha1Verifier,
};

const RECORD_VERSION: &str = "litterkeep-scram-sha1-v1";

/// Соль, стоимость KDF и производные ключи SCRAM без исходного пароля.
pub struct ScramSha1Record {
    salt: Vec<u8>,
    iterations: u32,
    verifier: ScramSha1Verifier,
}

impl ScramSha1Record {
    /// Выводит и упаковывает credential нового владельца.
    pub fn create(password: &[u8]) -> Option<Self> {
        let (salt, iterations, verifier) = ScramSha1Verifier::create(password)?;
        Some(Self {
            salt,
            iterations,
            verifier,
        })
    }

    /// Разбирает запись строго по версии, длинам и диапазону iteration count.
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
                verifier: ScramSha1Verifier::from_keys(stored_key, server_key),
            }),
            _ => None,
        }
    }

    /// Сериализует все поля в одной записи, пригодной для атомарной публикации.
    pub fn encode(&self) -> String {
        format!(
            "{RECORD_VERSION}${}${}${}${}\n",
            self.iterations,
            encode_hex(&self.salt),
            encode_hex(self.verifier.stored_key()),
            encode_hex(self.verifier.server_key())
        )
    }

    /// Возвращает salt, необходимый SCRAM server-first-message.
    pub fn salt(&self) -> &[u8] {
        &self.salt
    }

    /// Возвращает число итераций, необходимое SCRAM server-first-message.
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Возвращает криптографический verifier для проверки клиентского proof.
    pub fn verifier(&self) -> &ScramSha1Verifier {
        &self.verifier
    }
}

/// Отклоняет недопустимую соль или чрезмерную стоимость разбора attacker data.
fn valid_parameters(salt: &[u8], iterations: u32) -> bool {
    matches!(
        (salt.len(), iterations),
        (
            MIN_SALT_SIZE..=MAX_SALT_SIZE,
            MIN_ITERATIONS..=MAX_ITERATIONS
        )
    )
}

/// Декодирует hex без прямой индексации байтов входной строки.
fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

/// Декодирует ровно 20 октетов StoredKey или ServerKey.
fn decode_fixed_key(value: &str) -> Option<[u8; 20]> {
    let decoded = decode_hex(value)?;
    decoded.try_into().ok()
}

/// Преобразует один ASCII hex digit, отвергая прочие варианты.
fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Кодирует байты в lowercase hex для стабильного формата.
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

#[cfg(test)]
#[path = "scram_sha1_record_tests.rs"]
mod tests;
