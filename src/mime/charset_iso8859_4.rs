//! Таблица преобразования MIME charset ISO-8859-4 (Latin-4).

/// Декодирует ISO-8859-4 и отклоняет позиции без назначенного символа.
///
/// ASCII и C1 control range имеют одинаковое отображение во всех ISO-8859
/// таблицах. Для A0..FF используется точная таблица предпочтительного набора.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один октет без прямого индексирования таблицы.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .flatten()
                .ok_or("undefined ISO-8859-4 octet")
        }
    }
}

/// Назначения октетов A0..FF; `None` явно задаёт undefined octet.
const UPPER_HALF: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('\u{0104}'),
    Some('\u{0138}'),
    Some('\u{0156}'),
    Some('\u{00a4}'),
    None,
    Some('\u{013b}'),
    Some('\u{00a7}'),
    Some('\u{00a8}'),
    Some('\u{0160}'),
    Some('\u{0112}'),
    Some('\u{0122}'),
    Some('\u{0166}'),
    Some('\u{00ad}'),
    None,
    Some('\u{00af}'),
    Some('\u{00b0}'),
    Some('\u{0105}'),
    Some('\u{02db}'),
    Some('\u{0157}'),
    Some('\u{00b4}'),
    Some('\u{0129}'),
    Some('\u{013c}'),
    Some('\u{02c7}'),
    Some('\u{00b8}'),
    Some('\u{0161}'),
    Some('\u{0113}'),
    Some('\u{0123}'),
    Some('\u{0167}'),
    Some('\u{014a}'),
    None,
    Some('\u{014b}'),
    Some('\u{0100}'),
    Some('\u{00c1}'),
    Some('\u{00c2}'),
    None,
    Some('\u{00c4}'),
    Some('\u{00c5}'),
    Some('\u{00c6}'),
    Some('\u{012e}'),
    Some('\u{010c}'),
    Some('\u{00c9}'),
    Some('\u{0118}'),
    Some('\u{00cb}'),
    Some('\u{0116}'),
    Some('\u{00cd}'),
    Some('\u{00ce}'),
    Some('\u{012a}'),
    None,
    Some('\u{0145}'),
    Some('\u{014c}'),
    Some('\u{0136}'),
    Some('\u{00d4}'),
    Some('\u{00d5}'),
    Some('\u{00d6}'),
    Some('\u{00d7}'),
    Some('\u{00d8}'),
    Some('\u{0172}'),
    Some('\u{00da}'),
    Some('\u{00db}'),
    Some('\u{00dc}'),
    Some('\u{0168}'),
    Some('\u{016a}'),
    Some('\u{00df}'),
    Some('\u{0101}'),
    Some('\u{00e1}'),
    Some('\u{00e2}'),
    None,
    Some('\u{00e4}'),
    Some('\u{00e5}'),
    Some('\u{00e6}'),
    Some('\u{012f}'),
    Some('\u{010d}'),
    Some('\u{00e9}'),
    Some('\u{0119}'),
    Some('\u{00eb}'),
    Some('\u{0117}'),
    Some('\u{00ed}'),
    Some('\u{00ee}'),
    Some('\u{012b}'),
    None,
    Some('\u{0146}'),
    Some('\u{014d}'),
    Some('\u{0137}'),
    Some('\u{00f4}'),
    Some('\u{00f5}'),
    Some('\u{00f6}'),
    Some('\u{00f7}'),
    Some('\u{00f8}'),
    Some('\u{0173}'),
    Some('\u{00fa}'),
    Some('\u{00fb}'),
    Some('\u{00fc}'),
    Some('\u{0169}'),
    Some('\u{016b}'),
    Some('\u{02d9}'),
];

#[cfg(test)]
#[path = "charset_iso8859_4_tests.rs"]
mod tests;
