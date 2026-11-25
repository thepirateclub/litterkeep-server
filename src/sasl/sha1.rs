//! Собственная реализация SHA-1 для RFC 5802 SCRAM-SHA-1.
//!
//! SHA-1 здесь нужен для совместимости с обязательным SASL-механизмом RFC,
//! а не для подписей или защиты от collision атак. Новые форматы должны
//! предпочитать современные hash функции. Реализация использует только safe
//! Rust и не добавляет production dependency.

/// Размер digest в октетах, требуемый форматом SHA-1.
pub const DIGEST_SIZE: usize = 20;

/// Вычисляет SHA-1 digest входного сообщения по RFC 3174.
pub fn digest(message: &[u8]) -> [u8; DIGEST_SIZE] {
    let mut padded = message.to_vec();
    let bit_length = (message.len() as u64).wrapping_mul(8);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    let mut words = Vec::with_capacity(80);
    for block in padded.as_chunks::<64>().0 {
        compress(block, &mut state, &mut words);
    }
    let mut output = [0_u8; DIGEST_SIZE];
    for (chunk, word) in output.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    output
}

/// Расширяет schedule и выполняет 80 раундов одного 512-битного блока.
fn compress(block: &[u8], state: &mut [u32; 5], words: &mut Vec<u32>) {
    words.clear();
    for octets in block.as_chunks::<4>().0 {
        words.push(u32::from_be_bytes(*octets));
    }
    for round in 16..80 {
        let word = words.get(round - 3).copied().unwrap_or_default()
            ^ words.get(round - 8).copied().unwrap_or_default()
            ^ words.get(round - 14).copied().unwrap_or_default()
            ^ words.get(round - 16).copied().unwrap_or_default();
        words.push(word.rotate_left(1));
    }

    let [h0, h1, h2, h3, h4] = *state;
    let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
    for (round, word) in words.iter().enumerate() {
        let (function, constant) = round_function(round, b, c, d);
        let next = a
            .rotate_left(5)
            .wrapping_add(function)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(*word);
        (a, b, c, d, e) = (next, a, b.rotate_left(30), c, d);
    }
    *state = [
        h0.wrapping_add(a),
        h1.wrapping_add(b),
        h2.wrapping_add(c),
        h3.wrapping_add(d),
        h4.wrapping_add(e),
    ];
}

/// Выбирает нелинейную функцию и раундовую константу по диапазону SHA-1.
fn round_function(round: usize, b: u32, c: u32, d: u32) -> (u32, u32) {
    match round {
        0..=19 => ((b & c) | (!b & d), 0x5a82_7999),
        20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
        40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
        _ => (b ^ c ^ d, 0xca62_c1d6),
    }
}

#[cfg(test)]
#[path = "sha1_tests.rs"]
mod tests;
