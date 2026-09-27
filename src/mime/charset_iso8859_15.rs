//! Таблица MIME charset ISO-8859-15 (Latin-9).

/// Декодирует каждый октет ISO-8859-15 по таблице Unicode Consortium.
///
/// Профиль заменяет восемь редко используемых знаков Latin-1 на евро,
/// французские и финские буквы, сохраняя ASCII, C1 и остальные позиции.
pub(super) fn decode(bytes: &[u8]) -> String {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один октет без индексации внешнего ввода.
fn decode_byte(byte: u8) -> char {
    match byte {
        0x00..=0xa3 | 0xa5 | 0xa7 | 0xa9..=0xb3 | 0xb5..=0xb7 | 0xb9..=0xbb | 0xbf..=0xff => {
            char::from(byte)
        }
        0xa4 => '€',
        0xa6 => 'Š',
        0xa8 => 'š',
        0xb4 => 'Ž',
        0xb8 => 'ž',
        0xbc => 'Œ',
        0xbd => 'œ',
        0xbe => 'Ÿ',
    }
}

#[cfg(test)]
#[path = "charset_iso8859_15_tests.rs"]
mod tests;
