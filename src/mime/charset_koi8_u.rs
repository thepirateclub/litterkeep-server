//! Декодирование MIME charset KOI8-U по RFC 2319 и базовой таблице KOI8-R.

/// Декодирует вход за один проход, заменяя только восемь украинских позиций.
pub(super) fn decode(bytes: &[u8]) -> String {
    bytes.iter().copied().map(decode_octet).collect()
}

/// Применяет украинские overrides, переиспользуя нормативную KOI8-R таблицу.
fn decode_octet(octet: u8) -> char {
    match octet {
        0xa4 => '\u{0454}',
        0xa6 => '\u{0456}',
        0xa7 => '\u{0457}',
        0xad => '\u{0491}',
        0xb4 => '\u{0404}',
        0xb6 => '\u{0406}',
        0xb7 => '\u{0407}',
        0xbd => '\u{0490}',
        _ => super::charset_koi8_r::decode_octet(octet),
    }
}

#[cfg(test)]
#[path = "charset_koi8_u_tests.rs"]
mod tests;
