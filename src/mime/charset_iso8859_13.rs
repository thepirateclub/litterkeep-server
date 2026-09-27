//! Таблица MIME charset ISO-8859-13 (Baltic Rim).

/// Декодирует все значения однобайтового профиля ISO-8859-13.
///
/// Таблица полностью определена: lower-half controls отображаются как те же
/// Unicode scalar values, верхняя половина соответствует таблице Unicode.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует байт безопасным доступом к 96-символьной таблице.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .ok_or("ISO-8859-13 octet is outside the mapping")
        }
    }
}

/// Отображения A0..FF согласно Unicode Consortium mapping ISO-8859-13:1998.
const UPPER_HALF: [char; 96] = [
    '\u{00a0}', '\u{201d}', '\u{00a2}', '\u{00a3}', '\u{00a4}', '\u{201e}', '\u{00a6}', '\u{00a7}',
    '\u{00d8}', '\u{00a9}', '\u{0156}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{00c6}',
    '\u{00b0}', '\u{00b1}', '\u{00b2}', '\u{00b3}', '\u{201c}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{00f8}', '\u{00b9}', '\u{0157}', '\u{00bb}', '\u{00bc}', '\u{00bd}', '\u{00be}', '\u{00e6}',
    '\u{0104}', '\u{012e}', '\u{0100}', '\u{0106}', '\u{00c4}', '\u{00c5}', '\u{0118}', '\u{0112}',
    '\u{010c}', '\u{00c9}', '\u{0179}', '\u{0116}', '\u{0122}', '\u{0136}', '\u{012a}', '\u{013b}',
    '\u{0160}', '\u{0143}', '\u{0145}', '\u{00d3}', '\u{014c}', '\u{00d5}', '\u{00d6}', '\u{00d7}',
    '\u{0172}', '\u{0141}', '\u{015a}', '\u{016a}', '\u{00dc}', '\u{017b}', '\u{017d}', '\u{00df}',
    '\u{0105}', '\u{012f}', '\u{0101}', '\u{0107}', '\u{00e4}', '\u{00e5}', '\u{0119}', '\u{0113}',
    '\u{010d}', '\u{00e9}', '\u{017a}', '\u{0117}', '\u{0123}', '\u{0137}', '\u{012b}', '\u{013c}',
    '\u{0161}', '\u{0144}', '\u{0146}', '\u{00f3}', '\u{014d}', '\u{00f5}', '\u{00f6}', '\u{00f7}',
    '\u{0173}', '\u{0142}', '\u{015b}', '\u{016b}', '\u{00fc}', '\u{017c}', '\u{017e}', '\u{2019}',
];

#[cfg(test)]
#[path = "charset_iso8859_13_tests.rs"]
mod tests;
