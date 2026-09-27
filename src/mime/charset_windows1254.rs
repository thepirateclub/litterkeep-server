//! Декодер MIME charset Windows-1254 для турецкого текста.

/// Преобразует текст по таблице Microsoft и отклоняет undefined octets.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует octet строго по диапазону и таблице CP1254.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => lookup(&CONTROL_BLOCK, byte - 0x80),
        0xa0..=0xff => Ok(decode_graphic(byte)),
    }
}

/// Возвращает назначение таблицы без прямого индексирования массива.
fn lookup(table: &[Option<char>], offset: u8) -> Result<char, &'static str> {
    table
        .get(usize::from(offset))
        .copied()
        .flatten()
        .ok_or("undefined Windows-1254 octet")
}

/// Применяет турецкие замены к прямому Latin-1 диапазону A0..FF.
fn decode_graphic(byte: u8) -> char {
    match byte {
        0xd0 => 'Ğ',
        0xdd => 'İ',
        0xde => 'Ş',
        0xf0 => 'ğ',
        0xfd => 'ı',
        0xfe => 'ş',
        _ => char::from(byte),
    }
}

/// Назначения C1 по Microsoft CP1254; None отражает отверстия исходной таблицы.
const CONTROL_BLOCK: [Option<char>; 32] = [
    Some('€'),
    None,
    Some('‚'),
    Some('ƒ'),
    Some('„'),
    Some('…'),
    Some('†'),
    Some('‡'),
    Some('ˆ'),
    Some('‰'),
    Some('Š'),
    Some('‹'),
    Some('Œ'),
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
    Some('˜'),
    Some('™'),
    Some('š'),
    Some('›'),
    Some('œ'),
    None,
    None,
    Some('Ÿ'),
];

#[cfg(test)]
#[path = "charset_windows1254_tests.rs"]
mod tests;
