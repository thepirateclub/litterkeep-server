//! Таблица MIME charset TIS-620 и совместимого профиля ISO-8859-11.

/// Декодирует назначенные TIS-620 значения, отклоняя незаданные позиции.
///
/// ISO-8859-11 добавляет NBSP в A0 к TIS 620-2533; в обоих диапазон DB..DE
/// остаётся незаданным и не заменяется символом U+FFFD.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один байт безопасным чтением sparse mapping.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .flatten()
                .ok_or("undefined TIS-620 octet")
        }
    }
}

/// Отображения A0..FF согласно Unicode Consortium mapping.
const UPPER_HALF: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('\u{0e01}'),
    Some('\u{0e02}'),
    Some('\u{0e03}'),
    Some('\u{0e04}'),
    Some('\u{0e05}'),
    Some('\u{0e06}'),
    Some('\u{0e07}'),
    Some('\u{0e08}'),
    Some('\u{0e09}'),
    Some('\u{0e0a}'),
    Some('\u{0e0b}'),
    Some('\u{0e0c}'),
    Some('\u{0e0d}'),
    Some('\u{0e0e}'),
    Some('\u{0e0f}'),
    Some('\u{0e10}'),
    Some('\u{0e11}'),
    Some('\u{0e12}'),
    Some('\u{0e13}'),
    Some('\u{0e14}'),
    Some('\u{0e15}'),
    Some('\u{0e16}'),
    Some('\u{0e17}'),
    Some('\u{0e18}'),
    Some('\u{0e19}'),
    Some('\u{0e1a}'),
    Some('\u{0e1b}'),
    Some('\u{0e1c}'),
    Some('\u{0e1d}'),
    Some('\u{0e1e}'),
    Some('\u{0e1f}'),
    Some('\u{0e20}'),
    Some('\u{0e21}'),
    Some('\u{0e22}'),
    Some('\u{0e23}'),
    Some('\u{0e24}'),
    Some('\u{0e25}'),
    Some('\u{0e26}'),
    Some('\u{0e27}'),
    Some('\u{0e28}'),
    Some('\u{0e29}'),
    Some('\u{0e2a}'),
    Some('\u{0e2b}'),
    Some('\u{0e2c}'),
    Some('\u{0e2d}'),
    Some('\u{0e2e}'),
    Some('\u{0e2f}'),
    Some('\u{0e30}'),
    Some('\u{0e31}'),
    Some('\u{0e32}'),
    Some('\u{0e33}'),
    Some('\u{0e34}'),
    Some('\u{0e35}'),
    Some('\u{0e36}'),
    Some('\u{0e37}'),
    Some('\u{0e38}'),
    Some('\u{0e39}'),
    Some('\u{0e3a}'),
    None,
    None,
    None,
    None,
    Some('\u{0e3f}'),
    Some('\u{0e40}'),
    Some('\u{0e41}'),
    Some('\u{0e42}'),
    Some('\u{0e43}'),
    Some('\u{0e44}'),
    Some('\u{0e45}'),
    Some('\u{0e46}'),
    Some('\u{0e47}'),
    Some('\u{0e48}'),
    Some('\u{0e49}'),
    Some('\u{0e4a}'),
    Some('\u{0e4b}'),
    Some('\u{0e4c}'),
    Some('\u{0e4d}'),
    Some('\u{0e4e}'),
    Some('\u{0e4f}'),
    Some('\u{0e50}'),
    Some('\u{0e51}'),
    Some('\u{0e52}'),
    Some('\u{0e53}'),
    Some('\u{0e54}'),
    Some('\u{0e55}'),
    Some('\u{0e56}'),
    Some('\u{0e57}'),
    Some('\u{0e58}'),
    Some('\u{0e59}'),
    Some('\u{0e5a}'),
    Some('\u{0e5b}'),
    None,
    None,
    None,
    None,
];

#[cfg(test)]
#[path = "charset_tis620_tests.rs"]
mod tests;
