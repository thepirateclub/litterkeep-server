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

/// Секреты сервера для proof validation и server signature.
///
/// Пароль здесь уже обязан быть SASLprep-normalized вызывающим уровнем. Пока
/// profile normalization и account persistence не подключены, тип не выдаёт
/// себя за готовую SCRAM credential и не используется listener-ами.
pub struct ScramSha1Verifier {
    stored_key: [u8; 20],
    server_key: [u8; 20],
}

impl ScramSha1Verifier {
    /// Выводит verifier только для ограниченных salt и iteration параметров.
    pub fn derive(password: &[u8], salt: &[u8], iterations: u32) -> Option<Self> {
        if !valid_parameters(salt, iterations) {
            return None;
        }
        let salted_password = derive(password, salt, iterations)?;
        let client_key = authenticate(&salted_password, b"Client Key");
        let stored_key = digest(&client_key);
        let server_key = authenticate(&salted_password, b"Server Key");
        Some(Self {
            stored_key,
            server_key,
        })
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
