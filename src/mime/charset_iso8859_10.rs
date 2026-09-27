//! Таблица MIME charset ISO-8859-10 (Latin-6, Nordic).

/// Декодирует назначенные octets и отказывает только при ошибке внутреннего диапазона.
///
/// Все 256 значений ISO/IEC 8859-10 имеют Unicode mapping, включая controls.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует байт безопасным чтением таблицы старшей половины.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .ok_or("ISO-8859-10 octet is outside the mapping")
        }
    }
}

/// Таблица Unicode Consortium для ISO/IEC 8859-10:1998, байты A0..FF.
const UPPER_HALF: [char; 96] = [
    '\u{00a0}', '\u{0104}', '\u{0112}', '\u{0122}', '\u{012a}', '\u{0128}', '\u{0136}', '\u{00a7}',
    '\u{013b}', '\u{0110}', '\u{0160}', '\u{0166}', '\u{017d}', '\u{00ad}', '\u{016a}', '\u{014a}',
    '\u{00b0}', '\u{0105}', '\u{0113}', '\u{0123}', '\u{012b}', '\u{0129}', '\u{0137}', '\u{00b7}',
    '\u{013c}', '\u{0111}', '\u{0161}', '\u{0167}', '\u{017e}', '\u{2015}', '\u{016b}', '\u{014b}',
    '\u{0100}', '\u{00c1}', '\u{00c2}', '\u{00c3}', '\u{00c4}', '\u{00c5}', '\u{00c6}', '\u{012e}',
    '\u{010c}', '\u{00c9}', '\u{0118}', '\u{00cb}', '\u{0116}', '\u{00cd}', '\u{00ce}', '\u{00cf}',
    '\u{00d0}', '\u{0145}', '\u{014c}', '\u{00d3}', '\u{00d4}', '\u{00d5}', '\u{00d6}', '\u{0168}',
    '\u{00d8}', '\u{0172}', '\u{00da}', '\u{00db}', '\u{00dc}', '\u{00dd}', '\u{00de}', '\u{00df}',
    '\u{0101}', '\u{00e1}', '\u{00e2}', '\u{00e3}', '\u{00e4}', '\u{00e5}', '\u{00e6}', '\u{012f}',
    '\u{010d}', '\u{00e9}', '\u{0119}', '\u{00eb}', '\u{0117}', '\u{00ed}', '\u{00ee}', '\u{00ef}',
    '\u{00f0}', '\u{0146}', '\u{014d}', '\u{00f3}', '\u{00f4}', '\u{00f5}', '\u{00f6}', '\u{0169}',
    '\u{00f8}', '\u{0173}', '\u{00fa}', '\u{00fb}', '\u{00fc}', '\u{00fd}', '\u{00fe}', '\u{0138}',
];

#[cfg(test)]
#[path = "charset_iso8859_10_tests.rs"]
mod tests;
