//! HMAC-SHA-1, требуемый SCRAM-SHA-1 RFC 5802.
use super::sha1::{DIGEST_SIZE, digest};

/// Вычисляет RFC 2104 HMAC для SHA-1, включая ключи длиннее блока hash.
pub fn authenticate(key: &[u8], message: &[u8]) -> [u8; DIGEST_SIZE] {
    const BLOCK_SIZE: usize = 64;
    let normalized_key = match key.len() > BLOCK_SIZE {
        true => digest(key).to_vec(),
        false => key.to_vec(),
    };
    let mut inner_pad = vec![0x36; BLOCK_SIZE];
    let mut outer_pad = vec![0x5c; BLOCK_SIZE];
    for (slot, octet) in inner_pad.iter_mut().zip(&normalized_key) {
        *slot ^= octet;
    }
    for (slot, octet) in outer_pad.iter_mut().zip(&normalized_key) {
        *slot ^= octet;
    }
    inner_pad.extend_from_slice(message);
    outer_pad.extend_from_slice(&digest(&inner_pad));
    digest(&outer_pad)
}

#[cfg(test)]
#[path = "hmac_sha1_tests.rs"]
mod tests;
