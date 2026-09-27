//! Таблица MIME charset ISO-8859-9 (Latin-5, турецкий профиль).

/// Преобразует каждый octet в соответствующую Unicode scalar value.
///
/// ISO-8859-9 определяет всю таблицу, включая C0/C1 controls. Вариант заменяет
/// шесть позиций ISO-8859-1 турецкими буквами, остальные значения сохраняет.
pub(super) fn decode(bytes: &[u8]) -> String {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует байт посредством безопасного поиска в компактной upper-half таблице.
fn decode_byte(byte: u8) -> char {
    match byte {
        0x00..=0x9f => char::from(byte),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .unwrap_or(char::REPLACEMENT_CHARACTER)
        }
    }
}

/// Назначения A0..FF по таблице Unicode ISO/IEC 8859-9:1999.
const UPPER_HALF: [char; 96] = [
    '\u{00a0}', '\u{00a1}', '\u{00a2}', '\u{00a3}', '\u{00a4}', '\u{00a5}', '\u{00a6}', '\u{00a7}',
    '\u{00a8}', '\u{00a9}', '\u{00aa}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{00af}',
    '\u{00b0}', '\u{00b1}', '\u{00b2}', '\u{00b3}', '\u{00b4}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{00b8}', '\u{00b9}', '\u{00ba}', '\u{00bb}', '\u{00bc}', '\u{00bd}', '\u{00be}', '\u{00bf}',
    '\u{00c0}', '\u{00c1}', '\u{00c2}', '\u{00c3}', '\u{00c4}', '\u{00c5}', '\u{00c6}', '\u{00c7}',
    '\u{00c8}', '\u{00c9}', '\u{00ca}', '\u{00cb}', '\u{00cc}', '\u{00cd}', '\u{00ce}', '\u{00cf}',
    '\u{011e}', '\u{00d1}', '\u{00d2}', '\u{00d3}', '\u{00d4}', '\u{00d5}', '\u{00d6}', '\u{00d7}',
    '\u{00d8}', '\u{00d9}', '\u{00da}', '\u{00db}', '\u{00dc}', '\u{0130}', '\u{015e}', '\u{00df}',
    '\u{00e0}', '\u{00e1}', '\u{00e2}', '\u{00e3}', '\u{00e4}', '\u{00e5}', '\u{00e6}', '\u{00e7}',
    '\u{00e8}', '\u{00e9}', '\u{00ea}', '\u{00eb}', '\u{00ec}', '\u{00ed}', '\u{00ee}', '\u{00ef}',
    '\u{011f}', '\u{00f1}', '\u{00f2}', '\u{00f3}', '\u{00f4}', '\u{00f5}', '\u{00f6}', '\u{00f7}',
    '\u{00f8}', '\u{00f9}', '\u{00fa}', '\u{00fb}', '\u{00fc}', '\u{0131}', '\u{015f}', '\u{00ff}',
];

#[cfg(test)]
#[path = "charset_iso8859_9_tests.rs"]
mod tests;
