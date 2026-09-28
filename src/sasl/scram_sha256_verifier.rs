//! StoredKey/ServerKey verifier для SCRAM-SHA-256 по RFC 7677.

use super::{hmac_sha256::authenticate, pbkdf2_sha256::derive, sha256::digest};

/// Минимальный iteration count, объявляемый для SCRAM-SHA-256.
pub const MIN_ITERATIONS: u32 = 4096;
/// Верхняя локальная граница стоимости проверки одного пароля.
pub const MAX_ITERATIONS: u32 = 200_000;
/// Допустимый диапазон длины соли в локальном формате credentials.
pub const MIN_SALT_SIZE: usize = 1;
/// Допустимый диапазон длины соли в локальном формате credentials.
pub const MAX_SALT_SIZE: usize = 64;
/// Размер соли при создании новых SCRAM credentials.
pub const DEFAULT_SALT_SIZE: usize = 16;
/// Ограничивает размер пароля до SASLprep и вычислительного KDF.
pub const MAX_PASSWORD_SIZE: usize = 4096;

/// Содержит только производные ключи, исходный пароль не сохраняется.
pub struct ScramSha256Verifier {
    stored_key: [u8; 32],
    server_key: [u8; 32],
}

impl ScramSha256Verifier {
    /// Создаёт verifier со случайной солью и минимальной совместимой cost.
    pub fn create(password: &[u8]) -> Option<(Vec<u8>, u32, Self)> {
        let salt = argon2::password_hash::try_generate_salt().ok()?;
        let verifier = Self::derive(password, &salt, MIN_ITERATIONS)?;
        Some((salt.to_vec(), MIN_ITERATIONS, verifier))
    }

    /// Выводит SCRAM ключи только после проверки локальных параметров.
    pub fn derive(password: &[u8], salt: &[u8], iterations: u32) -> Option<Self> {
        if password.len() > MAX_PASSWORD_SIZE || !valid_parameters(salt, iterations) {
            return None;
        }
        let password_text = std::str::from_utf8(password).ok()?;
        let prepared =
            super::saslprep::prepare(password_text, super::saslprep::SaslPrepContext::StoredValue)
                .ok()?;
        let salted_password = derive(prepared.as_bytes(), salt, iterations)?;
        let client_key = authenticate(&salted_password, b"Client Key");
        Some(Self {
            stored_key: digest(&client_key),
            server_key: authenticate(&salted_password, b"Server Key"),
        })
    }

    /// Восстанавливает verifier только из уже валидированной записи.
    pub fn from_keys(stored_key: [u8; 32], server_key: [u8; 32]) -> Self {
        Self {
            stored_key,
            server_key,
        }
    }

    /// Проверяет proof constant-time относительно содержимого StoredKey.
    pub fn verify_client_proof(&self, auth_message: &[u8], proof: &[u8]) -> bool {
        if proof.len() != self.stored_key.len() {
            return false;
        }
        let signature = authenticate(&self.stored_key, auth_message);
        let mut client_key = [0_u8; 32];
        for ((recovered, proof_octet), signature_octet) in client_key
            .iter_mut()
            .zip(proof.iter().copied())
            .zip(signature)
        {
            *recovered = proof_octet ^ signature_octet;
        }
        constant_time_equal(&digest(&client_key), &self.stored_key)
    }

    /// Строит server signature для точного SCRAM AuthMessage.
    pub fn server_signature(&self, auth_message: &[u8]) -> [u8; 32] {
        authenticate(&self.server_key, auth_message)
    }

    /// Возвращает StoredKey для проверки client proof.
    pub fn stored_key(&self) -> &[u8; 32] {
        &self.stored_key
    }

    /// Возвращает ServerKey для server signature.
    pub fn server_key(&self) -> &[u8; 32] {
        &self.server_key
    }
}

/// Сравнивает весь ключевой материал без раннего выхода при несовпадении.
fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for (left_octet, right_octet) in left.iter().zip(right) {
        difference |= left_octet ^ right_octet;
    }
    difference == 0
}

/// Отклоняет недопустимые параметры до запуска PBKDF2.
pub(super) fn valid_parameters(salt: &[u8], iterations: u32) -> bool {
    matches!(
        (salt.len(), iterations),
        (
            MIN_SALT_SIZE..=MAX_SALT_SIZE,
            MIN_ITERATIONS..=MAX_ITERATIONS
        )
    )
}

#[cfg(test)]
#[path = "scram_sha256_verifier_tests.rs"]
mod tests;
