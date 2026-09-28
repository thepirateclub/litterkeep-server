//! Одноблочный PBKDF2-HMAC-SHA-256 для 32-октетного SCRAM ключа.

use super::{hmac_sha256::authenticate, sha256::DIGEST_SIZE};

/// Ограничивает стоимость KDF до создания будущего protocol verifier.
pub const MAX_ITERATIONS: u32 = 200_000;

/// Вычисляет PBKDF2 block 1; нулевая стоимость отклоняется без результата.
pub fn derive(password: &[u8], salt: &[u8], iterations: u32) -> Option<[u8; DIGEST_SIZE]> {
    if !matches!(iterations, 1..=MAX_ITERATIONS) {
        return None;
    }
    let mut input = salt.to_vec();
    input.extend_from_slice(&1_u32.to_be_bytes());
    let first = authenticate(password, &input);
    let mut output = first;
    let mut previous = first;
    for _ in 1..iterations {
        previous = authenticate(password, &previous);
        for (target, octet) in output.iter_mut().zip(previous) {
            *target ^= octet;
        }
    }
    Some(output)
}

#[cfg(test)]
#[path = "pbkdf2_sha256_tests.rs"]
mod tests;
