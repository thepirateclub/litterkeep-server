//! Таблица преобразования MIME charset ISO-8859-3 (Latin-3).

/// Декодирует ISO-8859-3, отклоняя undefined позиции таблицы.
///
/// Нижние 160 октетов соответствуют ASCII и C1 control range. Значения
/// A0..FF заданы отдельно; `None` означает нормативно незаданный октет.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует байт через безопасный поиск без прямой индексации таблицы.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .flatten()
                .ok_or("undefined ISO-8859-3 octet")
        }
    }
}

/// Таблица назначений байтов A0..FF; `None` означает undefined code point.
const UPPER_HALF: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('\u{0126}'),
    Some('\u{02d8}'),
    Some('\u{00a3}'),
    Some('\u{00a4}'),
    None,
    Some('\u{0124}'),
    Some('\u{00a7}'),
    Some('\u{00a8}'),
    Some('\u{0130}'),
    Some('\u{015e}'),
    Some('\u{011e}'),
    Some('\u{0134}'),
    Some('\u{00ad}'),
    None,
    Some('\u{017b}'),
    Some('\u{00b0}'),
    Some('\u{0127}'),
    Some('\u{00b2}'),
    Some('\u{00b3}'),
    Some('\u{00b4}'),
    Some('\u{00b5}'),
    Some('\u{0125}'),
    Some('\u{00b7}'),
    Some('\u{00b8}'),
    Some('\u{0131}'),
    Some('\u{015f}'),
    Some('\u{011f}'),
    Some('\u{0135}'),
    Some('\u{00bd}'),
    None,
    Some('\u{017c}'),
    Some('\u{00c0}'),
    Some('\u{00c1}'),
    Some('\u{00c2}'),
    None,
    Some('\u{00c4}'),
    Some('\u{010a}'),
    Some('\u{0108}'),
    Some('\u{00c7}'),
    Some('\u{00c8}'),
    Some('\u{00c9}'),
    Some('\u{00ca}'),
    Some('\u{00cb}'),
    Some('\u{00cc}'),
    Some('\u{00cd}'),
    Some('\u{00ce}'),
    Some('\u{00cf}'),
    None,
    Some('\u{00d1}'),
    Some('\u{00d2}'),
    Some('\u{00d3}'),
    Some('\u{00d4}'),
    Some('\u{0120}'),
    Some('\u{00d6}'),
    Some('\u{00d7}'),
    Some('\u{011c}'),
    Some('\u{00d9}'),
    Some('\u{00da}'),
    Some('\u{00db}'),
    Some('\u{00dc}'),
    Some('\u{016c}'),
    Some('\u{015c}'),
    Some('\u{00df}'),
    Some('\u{00e0}'),
    Some('\u{00e1}'),
    Some('\u{00e2}'),
    None,
    Some('\u{00e4}'),
    Some('\u{010b}'),
    Some('\u{0109}'),
    Some('\u{00e7}'),
    Some('\u{00e8}'),
    Some('\u{00e9}'),
    Some('\u{00ea}'),
    Some('\u{00eb}'),
    Some('\u{00ec}'),
    Some('\u{00ed}'),
    Some('\u{00ee}'),
    Some('\u{00ef}'),
    None,
    Some('\u{00f1}'),
    Some('\u{00f2}'),
    Some('\u{00f3}'),
    Some('\u{00f4}'),
    Some('\u{0121}'),
    Some('\u{00f6}'),
    Some('\u{00f7}'),
    Some('\u{011d}'),
    Some('\u{00f9}'),
    Some('\u{00fa}'),
    Some('\u{00fb}'),
    Some('\u{00fc}'),
    Some('\u{016d}'),
    Some('\u{015d}'),
    Some('\u{02d9}'),
];

#[cfg(test)]
#[path = "charset_iso8859_3_tests.rs"]
mod tests;
