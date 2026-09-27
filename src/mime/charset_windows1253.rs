//! Таблица MIME charset Windows-1253 для греческого текста.

/// Декодирует назначения Microsoft Windows-1253 без угадывания holes.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один octet из ASCII или одной из проверенных таблиц.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => lookup(&CONTROL_BLOCK, byte - 0x80),
        0xa0..=0xff => lookup(&GRAPHIC_BLOCK, byte - 0xa0),
    }
}

/// Читает значение таблицы по ограниченному смещению без indexing operator.
fn lookup(table: &[Option<char>], offset: u8) -> Result<char, &'static str> {
    table
        .get(usize::from(offset))
        .copied()
        .flatten()
        .ok_or("undefined Windows-1253 octet")
}

/// Назначения блока C1 по официальной таблице Microsoft CP1253.
const CONTROL_BLOCK: [Option<char>; 32] = [
    Some('€'),
    None,
    Some('‚'),
    Some('ƒ'),
    Some('„'),
    Some('…'),
    Some('†'),
    Some('‡'),
    None,
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
    None,
    Some('™'),
    None,
    Some('›'),
    None,
    None,
    None,
    None,
];

/// Назначения графического диапазона A0..FF Windows-1253.
const GRAPHIC_BLOCK: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('΅'),
    Some('Ά'),
    Some('£'),
    Some('¤'),
    Some('¥'),
    Some('¦'),
    Some('§'),
    Some('¨'),
    Some('©'),
    None,
    Some('«'),
    Some('¬'),
    Some('\u{00ad}'),
    Some('®'),
    Some('―'),
    Some('°'),
    Some('±'),
    Some('²'),
    Some('³'),
    Some('΄'),
    Some('µ'),
    Some('¶'),
    Some('·'),
    Some('Έ'),
    Some('Ή'),
    Some('Ί'),
    Some('»'),
    Some('Ό'),
    Some('½'),
    Some('Ύ'),
    Some('Ώ'),
    Some('ΐ'),
    Some('Α'),
    Some('Β'),
    Some('Γ'),
    Some('Δ'),
    Some('Ε'),
    Some('Ζ'),
    Some('Η'),
    Some('Θ'),
    Some('Ι'),
    Some('Κ'),
    Some('Λ'),
    Some('Μ'),
    Some('Ν'),
    Some('Ξ'),
    Some('Ο'),
    Some('Π'),
    Some('Ρ'),
    None,
    Some('Σ'),
    Some('Τ'),
    Some('Υ'),
    Some('Φ'),
    Some('Χ'),
    Some('Ψ'),
    Some('Ω'),
    Some('Ϊ'),
    Some('Ϋ'),
    Some('ά'),
    Some('έ'),
    Some('ή'),
    Some('ί'),
    Some('ΰ'),
    Some('α'),
    Some('β'),
    Some('γ'),
    Some('δ'),
    Some('ε'),
    Some('ζ'),
    Some('η'),
    Some('θ'),
    Some('ι'),
    Some('κ'),
    Some('λ'),
    Some('μ'),
    Some('ν'),
    Some('ξ'),
    Some('ο'),
    Some('π'),
    Some('ρ'),
    Some('ς'),
    Some('σ'),
    Some('τ'),
    Some('υ'),
    Some('φ'),
    Some('χ'),
    Some('ψ'),
    Some('ω'),
    Some('ϊ'),
    Some('ϋ'),
    Some('ό'),
    Some('ύ'),
    Some('ώ'),
    None,
];

#[cfg(test)]
#[path = "charset_windows1253_tests.rs"]
mod tests;
