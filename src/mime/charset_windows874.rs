//! Декодер IANA MIME charset Windows-874 для тайского письма.

/// Декодирует ASCII и назначенные Microsoft octets без подмены holes.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Выбирает ASCII, C1 punctuation или Thai code-point ranges.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => lookup(&CONTROL_BLOCK, byte - 0x80),
        0xa0 => Ok('\u{00a0}'),
        0xa1..=0xda => thai_character(0x0e01, byte - 0xa1),
        0xdb..=0xde | 0xfc..=0xff => Err("undefined Windows-874 octet"),
        0xdf => Ok('\u{0e3f}'),
        0xe0..=0xfb => thai_character(0x0e40, byte - 0xe0),
    }
}

/// Вычисляет scalar внутри последовательных назначений Thai в CP874.
fn thai_character(start: u32, offset: u8) -> Result<char, &'static str> {
    char::from_u32(start + u32::from(offset)).ok_or("invalid Windows-874 scalar")
}

/// Читает редкую C1 таблицу без прямого индексирования.
fn lookup(table: &[Option<char>], offset: u8) -> Result<char, &'static str> {
    table
        .get(usize::from(offset))
        .copied()
        .flatten()
        .ok_or("undefined Windows-874 octet")
}

/// Microsoft CP874 назначает в C1 только евро, многоточие и кавычки.
const CONTROL_BLOCK: [Option<char>; 32] = [
    Some('€'),
    None,
    None,
    None,
    None,
    Some('…'),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some('‘'),
    Some('’'),
    Some('“'),
    Some('”'),
    Some('•'),
    Some('–'),
    Some('—'),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];

#[cfg(test)]
#[path = "charset_windows874_tests.rs"]
mod tests;
