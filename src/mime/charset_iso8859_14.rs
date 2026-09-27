//! Таблица MIME charset ISO-8859-14 (Latin-8, Celtic).

/// Декодирует все байты ISO-8859-14 в соответствии с Unicode mapping.
///
/// Профиль полностью определён, включая C0/C1 controls; Celtic letters
/// располагаются в тех позициях Latin-1, где отсутствовали нужные знаки.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует байт безопасным чтением отдельной upper-half таблицы.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .ok_or("ISO-8859-14 octet is outside the mapping")
        }
    }
}

/// Назначения байтов A0..FF согласно ISO/IEC 8859-14:1998.
const UPPER_HALF: [char; 96] = [
    '\u{00a0}', '\u{1e02}', '\u{1e03}', '\u{00a3}', '\u{010a}', '\u{010b}', '\u{1e0a}', '\u{00a7}',
    '\u{1e80}', '\u{00a9}', '\u{1e82}', '\u{1e0b}', '\u{1ef2}', '\u{00ad}', '\u{00ae}', '\u{0178}',
    '\u{1e1e}', '\u{1e1f}', '\u{0120}', '\u{0121}', '\u{1e40}', '\u{1e41}', '\u{00b6}', '\u{1e56}',
    '\u{1e81}', '\u{1e57}', '\u{1e83}', '\u{1e60}', '\u{1ef3}', '\u{1e84}', '\u{1e85}', '\u{1e61}',
    '\u{00c0}', '\u{00c1}', '\u{00c2}', '\u{00c3}', '\u{00c4}', '\u{00c5}', '\u{00c6}', '\u{00c7}',
    '\u{00c8}', '\u{00c9}', '\u{00ca}', '\u{00cb}', '\u{00cc}', '\u{00cd}', '\u{00ce}', '\u{00cf}',
    '\u{0174}', '\u{00d1}', '\u{00d2}', '\u{00d3}', '\u{00d4}', '\u{00d5}', '\u{00d6}', '\u{1e6a}',
    '\u{00d8}', '\u{00d9}', '\u{00da}', '\u{00db}', '\u{00dc}', '\u{00dd}', '\u{0176}', '\u{00df}',
    '\u{00e0}', '\u{00e1}', '\u{00e2}', '\u{00e3}', '\u{00e4}', '\u{00e5}', '\u{00e6}', '\u{00e7}',
    '\u{00e8}', '\u{00e9}', '\u{00ea}', '\u{00eb}', '\u{00ec}', '\u{00ed}', '\u{00ee}', '\u{00ef}',
    '\u{0175}', '\u{00f1}', '\u{00f2}', '\u{00f3}', '\u{00f4}', '\u{00f5}', '\u{00f6}', '\u{1e6b}',
    '\u{00f8}', '\u{00f9}', '\u{00fa}', '\u{00fb}', '\u{00fc}', '\u{00fd}', '\u{0177}', '\u{00ff}',
];

#[cfg(test)]
#[path = "charset_iso8859_14_tests.rs"]
mod tests;
