//! Декодер зарегистрированной кириллической MIME-кодировки IBM866.

#[cfg(test)]
#[path = "charset_ibm866_tests.rs"]
mod tests;

/// Преобразует CP866 в Unicode по полной таблице Windows-совместимого профиля.
pub(super) fn decode(bytes: &[u8]) -> String {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Декодирует один байт, сохраняя ASCII и точные назначения старшей половины.
fn decode_byte(byte: u8) -> char {
    match byte {
        0x00..=0x7f => char::from(byte),
        0x80..=0x9f => cyrillic_character(0x0410, byte - 0x80),
        0xa0..=0xaf => cyrillic_character(0x0430, byte - 0xa0),
        0xb0..=0xdf => lookup(&BOX_AND_SHADE, byte - 0xb0),
        0xe0..=0xef => cyrillic_character(0x0440, byte - 0xe0),
        0xf0 => 'Ё',
        0xf1 => 'ё',
        0xf2 => 'Є',
        0xf3 => 'є',
        0xf4..=0xfc => lookup(&PUNCTUATION, byte - 0xf4),
        0xfd => '¤',
        0xfe => '■',
        0xff => ' ',
    }
}

/// Строит последовательный кириллический scalar без табличной индексации.
fn cyrillic_character(start: u32, offset: u8) -> char {
    char::from_u32(start + u32::from(offset)).unwrap_or(char::REPLACEMENT_CHARACTER)
}

/// Читает малую таблицу исключительно через bounds-checked `get`.
fn lookup(table: &[char], offset: u8) -> char {
    table
        .get(usize::from(offset))
        .copied()
        .unwrap_or(char::REPLACEMENT_CHARACTER)
}

/// CP866 box-drawing glyphs и shading в порядке исходной таблицы Microsoft.
const BOX_AND_SHADE: [char; 48] = [
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', '└', '┴', '┬',
    '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', '╨', '╤', '╥', '╙', '╘', '╒',
    '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀',
];

/// Знаки валют, пунктуация и псевдографика верхнего хвоста CP866.
const PUNCTUATION: [char; 9] = ['Ї', 'ї', 'Ў', 'ў', '°', '∙', '·', '√', '№'];
