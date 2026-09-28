//! SCRAM-SHA-1 StoredKey и ServerKey, полученные из RFC 5802 SaltedPassword.
use super::{hmac_sha1::authenticate, pbkdf2_sha1::derive, sha1::digest};

/// Минимум iteration count для SCRAM-SHA-1, совместимый с RFC 5802 профилем.
pub const MIN_ITERATIONS: u32 = 4096;
/// Верхняя граница вычислений, чтобы verifier не создавал неограниченную цену.
pub const MAX_ITERATIONS: u32 = 200_000;
/// Минимальный непустой salt, допускаемый локальной verifier policy.
pub const MIN_SALT_SIZE: usize = 1;
/// Максимальный размер SCRAM salt, принимаемого локальным verifier API.
pub const MAX_SALT_SIZE: usize = 64;
/// Рекомендуемый размер соли для будущего локального создания аккаунтов.
pub const DEFAULT_SALT_SIZE: usize = 16;
/// Ограничивает объём пароля до Unicode-подготовки и дорогостоящего KDF.
pub const MAX_PASSWORD_SIZE: usize = 4096;

/// Ключевой материал SCRAM-SHA-1 для проверки proof и подписи сервера.
///
/// Тип предоставляет crypto operations, но сам по себе не задаёт долговечный
/// credential format, nonce policy или состояние полного SASL exchange.
pub struct ScramSha1Verifier {
    stored_key: [u8; 20],
    server_key: [u8; 20],
}

impl ScramSha1Verifier {
    /// Создаёт verifier с криптографически случайной солью для нового аккаунта.
    pub fn create(password: &[u8]) -> Option<(Vec<u8>, u32, Self)> {
        let salt = argon2::password_hash::try_generate_salt().ok()?;
        let verifier = Self::derive(password, &salt, MIN_ITERATIONS)?;
        Some((salt.to_vec(), MIN_ITERATIONS, verifier))
    }

    /// Выводит verifier только для ограниченных salt и iteration параметров.
    pub fn derive(password: &[u8], salt: &[u8], iterations: u32) -> Option<Self> {
        if password.len() > MAX_PASSWORD_SIZE {
            return None;
        }
        if !valid_parameters(salt, iterations) {
            return None;
        }
        let password_text = std::str::from_utf8(password).ok()?;
        let normalized_password =
            super::saslprep::prepare(password_text, super::saslprep::SaslPrepContext::StoredValue)
                .ok()?;
        let salted_password = derive(normalized_password.as_bytes(), salt, iterations)?;
        let client_key = authenticate(&salted_password, b"Client Key");
        let stored_key = digest(&client_key);
        let server_key = authenticate(&salted_password, b"Server Key");
        Some(Self {
            stored_key,
            server_key,
        })
    }

    /// Создаёт verifier из ключей, прочитанных из проверенной account-записи.
    pub fn from_keys(stored_key: [u8; 20], server_key: [u8; 20]) -> Self {
        Self {
            stored_key,
            server_key,
        }
    }

    /// Проверяет SCRAM client proof без раннего выхода по содержимому ключа.
    pub fn verify_client_proof(&self, auth_message: &[u8], proof: &[u8]) -> bool {
        if proof.len() != self.stored_key.len() {
            return false;
        }
        let signature = authenticate(&self.stored_key, auth_message);
        let mut client_key = [0_u8; 20];
        for ((recovered, proof_octet), signature_octet) in client_key
            .iter_mut()
            .zip(proof.iter().copied())
            .zip(signature)
        {
            *recovered = proof_octet ^ signature_octet;
        }
        constant_time_equal(&digest(&client_key), &self.stored_key)
    }

    /// Строит server verifier для тех же байтов AuthMessage, что видел клиент.
    pub fn server_signature(&self, auth_message: &[u8]) -> [u8; 20] {
        authenticate(&self.server_key, auth_message)
    }

    /// Возвращает StoredKey для проверки клиентского proof.
    pub fn stored_key(&self) -> &[u8; 20] {
        &self.stored_key
    }

    /// Возвращает ServerKey для вычисления подписи сервера.
    pub fn server_key(&self) -> &[u8; 20] {
        &self.server_key
    }
}

/// Сравнивает все октеты, не прекращая цикл на первом несовпадении.
fn constant_time_equal(left: &[u8; 20], right: &[u8; 20]) -> bool {
    let mut difference = 0_u8;
    for (left_octet, right_octet) in left.iter().zip(right) {
        difference |= left_octet ^ right_octet;
    }
    difference == 0
}

/// Проверяет параметры до дорогостоящего PBKDF2 вычисления.
fn valid_parameters(salt: &[u8], iterations: u32) -> bool {
    matches!(
        (salt.len(), iterations),
        (
            MIN_SALT_SIZE..=MAX_SALT_SIZE,
            MIN_ITERATIONS..=MAX_ITERATIONS
        )
    )
}

#[cfg(test)]
#[path = "scram_sha1_verifier_tests.rs"]
mod tests;
