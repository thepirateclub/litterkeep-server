//! Собственная реализация SHA-256 по FIPS 180-4 и RFC 6234.

/// Размер SHA-256 digest в октетах.
pub const DIGEST_SIZE: usize = 32;

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Вычисляет SHA-256 для произвольного октетного сообщения.
pub fn digest(message: &[u8]) -> [u8; DIGEST_SIZE] {
    let mut padded = message.to_vec();
    let bit_length = u64::try_from(message.len())
        .unwrap_or(u64::MAX)
        .wrapping_mul(8);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut words = Vec::with_capacity(64);
    for block in padded.chunks_exact(64) {
        compress(block, &mut state, &mut words);
    }
    let mut output = [0_u8; DIGEST_SIZE];
    for (chunk, word) in output.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    output
}

/// Расширяет schedule и обрабатывает один 512-битный блок сообщения.
fn compress(block: &[u8], state: &mut [u32; 8], words: &mut Vec<u32>) {
    words.clear();
    for octets in block.chunks_exact(4) {
        let bytes: [u8; 4] = octets.try_into().expect("four-octet word");
        words.push(u32::from_be_bytes(bytes));
    }
    for round in 16..64 {
        let small_sigma_zero = words.get(round - 15).copied().unwrap_or_default();
        let small_sigma_one = words.get(round - 2).copied().unwrap_or_default();
        let sigma_zero = small_sigma_zero.rotate_right(7)
            ^ small_sigma_zero.rotate_right(18)
            ^ (small_sigma_zero >> 3);
        let sigma_one = small_sigma_one.rotate_right(17)
            ^ small_sigma_one.rotate_right(19)
            ^ (small_sigma_one >> 10);
        let word = sigma_one
            .wrapping_add(sigma_zero)
            .wrapping_add(words.get(round - 16).copied().unwrap_or_default())
            .wrapping_add(words.get(round - 7).copied().unwrap_or_default());
        words.push(word);
    }

    let [h0, h1, h2, h3, h4, h5, h6, h7] = *state;
    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (h0, h1, h2, h3, h4, h5, h6, h7);
    for (constant, word) in ROUND_CONSTANTS.iter().zip(words.iter()) {
        let choice = (e & f) ^ (!e & g);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let sigma_zero = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let sigma_one = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let temp_one = h
            .wrapping_add(sigma_one)
            .wrapping_add(choice)
            .wrapping_add(*constant)
            .wrapping_add(*word);
        let temp_two = sigma_zero.wrapping_add(majority);
        (a, b, c, d, e, f, g, h) = (
            temp_one.wrapping_add(temp_two),
            a,
            b,
            c,
            d.wrapping_add(temp_one),
            e,
            f,
            g,
        );
    }
    *state = [
        h0.wrapping_add(a),
        h1.wrapping_add(b),
        h2.wrapping_add(c),
        h3.wrapping_add(d),
        h4.wrapping_add(e),
        h5.wrapping_add(f),
        h6.wrapping_add(g),
        h7.wrapping_add(h),
    ];
}

#[cfg(test)]
#[path = "sha256_tests.rs"]
mod tests;
