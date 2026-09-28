//! PBKDF2-HMAC-SHA-1 block для SCRAM-SHA-1 RFC 5802.
use super::{hmac_sha1::authenticate, sha1::DIGEST_SIZE};

/// Вычисляет один PBKDF2 block, достаточный для 20-байтового SCRAM key.
///
/// Zero iterations не являются допустимым PBKDF2 параметром; для них функция
/// возвращает `None`, чтобы вызывающий протокольный слой отказал без fallback.
pub fn derive(password: &[u8], salt: &[u8], iterations: u32) -> Option<[u8; DIGEST_SIZE]> {
    if iterations == 0 {
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
#[path = "pbkdf2_sha1_tests.rs"]
mod tests;
