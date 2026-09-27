//! Декодер MIME charset Windows-1255 для Hebrew и RTL marks.

/// Декодирует байты по таблице Microsoft, отвергая все undefined positions.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Выбирает ASCII, C1 или графическое отображение без индексирования.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => lookup(&CONTROL_BLOCK, byte - 0x80),
        0xa0..=0xff => lookup(&GRAPHIC_BLOCK, byte - 0xa0),
    }
}

/// Возвращает назначение только для проверенного смещения таблицы.
fn lookup(table: &[Option<char>], offset: u8) -> Result<char, &'static str> {
    table
        .get(usize::from(offset))
        .copied()
        .flatten()
        .ok_or("undefined Windows-1255 octet")
}

/// Назначения диапазона 80..9F в Microsoft CP1255.
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
    None,
    Some('‹'),
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
    Some('˜'),
    Some('™'),
    None,
    Some('›'),
    None,
    None,
    None,
    None,
];

/// Назначения диапазона A0..FF, включая Hebrew и направления текста.
const GRAPHIC_BLOCK: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('¡'),
    Some('¢'),
    Some('£'),
    Some('₪'),
    Some('¥'),
    Some('¦'),
    Some('§'),
    Some('¨'),
    Some('©'),
    Some('×'),
    Some('«'),
    Some('¬'),
    Some('\u{00ad}'),
    Some('®'),
    Some('¯'),
    Some('°'),
    Some('±'),
    Some('²'),
    Some('³'),
    Some('´'),
    Some('µ'),
    Some('¶'),
    Some('·'),
    Some('¸'),
    Some('¹'),
    Some('÷'),
    Some('»'),
    Some('¼'),
    Some('½'),
    Some('¾'),
    Some('¿'),
    Some('\u{05b0}'),
    Some('\u{05b1}'),
    Some('\u{05b2}'),
    Some('\u{05b3}'),
    Some('\u{05b4}'),
    Some('\u{05b5}'),
    Some('\u{05b6}'),
    Some('\u{05b7}'),
    Some('\u{05b8}'),
    Some('\u{05b9}'),
    None,
    Some('\u{05bb}'),
    Some('\u{05bc}'),
    Some('\u{05bd}'),
    Some('\u{05be}'),
    Some('\u{05bf}'),
    Some('\u{05c0}'),
    Some('\u{05c1}'),
    Some('\u{05c2}'),
    Some('\u{05c3}'),
    Some('\u{05f0}'),
    Some('\u{05f1}'),
    Some('\u{05f2}'),
    Some('\u{05f3}'),
    Some('\u{05f4}'),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some('\u{05d0}'),
    Some('\u{05d1}'),
    Some('\u{05d2}'),
    Some('\u{05d3}'),
    Some('\u{05d4}'),
    Some('\u{05d5}'),
    Some('\u{05d6}'),
    Some('\u{05d7}'),
    Some('\u{05d8}'),
    Some('\u{05d9}'),
    Some('\u{05da}'),
    Some('\u{05db}'),
    Some('\u{05dc}'),
    Some('\u{05dd}'),
    Some('\u{05de}'),
    Some('\u{05df}'),
    Some('\u{05e0}'),
    Some('\u{05e1}'),
    Some('\u{05e2}'),
    Some('\u{05e3}'),
    Some('\u{05e4}'),
    Some('\u{05e5}'),
    Some('\u{05e6}'),
    Some('\u{05e7}'),
    Some('\u{05e8}'),
    Some('\u{05e9}'),
    Some('\u{05ea}'),
    None,
    None,
    Some('\u{200e}'),
    Some('\u{200f}'),
    None,
];

#[cfg(test)]
#[path = "charset_windows1255_tests.rs"]
mod tests;
