//! Таблица MIME charset Windows-1250 для центральноевропейского текста.

/// Декодирует назначенные Windows-1250 октеты и строго отвергает пять дыр.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один октет по таблице Microsoft, не угадывая undefined bytes.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => UPPER_CONTROL_BLOCK
            .get(usize::from(byte - 0x80))
            .copied()
            .flatten()
            .ok_or("undefined Windows-1250 octet"),
        0xa0..=0xff => UPPER_GRAPHIC_BLOCK
            .get(usize::from(byte - 0xa0))
            .copied()
            .ok_or("Windows-1250 octet is outside the mapping"),
    }
}

/// Назначения 0x80..0x9F; `None` соответствует дыре исходной таблицы.
const UPPER_CONTROL_BLOCK: [Option<char>; 32] = [
    Some('€'),
    None,
    Some('‚'),
    None,
    Some('„'),
    Some('…'),
    Some('†'),
    Some('‡'),
    None,
    Some('‰'),
    Some('Š'),
    Some('‹'),
    Some('Ś'),
    Some('Ť'),
    Some('Ž'),
    Some('Ź'),
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
    Some('š'),
    Some('›'),
    Some('ś'),
    Some('ť'),
    Some('ž'),
    Some('ź'),
];

/// Назначения A0..FF согласно официальной таблице Windows-1250.
const UPPER_GRAPHIC_BLOCK: [char; 96] = [
    '\u{00a0}', '\u{02c7}', '\u{02d8}', '\u{0141}', '\u{00a4}', '\u{0104}', '\u{00a6}', '\u{00a7}',
    '\u{00a8}', '\u{00a9}', '\u{015e}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{017b}',
    '\u{00b0}', '\u{00b1}', '\u{02db}', '\u{0142}', '\u{00b4}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{00b8}', '\u{0105}', '\u{015f}', '\u{00bb}', '\u{013d}', '\u{02dd}', '\u{013e}', '\u{017c}',
    '\u{0154}', '\u{00c1}', '\u{00c2}', '\u{0102}', '\u{00c4}', '\u{0139}', '\u{0106}', '\u{00c7}',
    '\u{010c}', '\u{00c9}', '\u{0118}', '\u{00cb}', '\u{011a}', '\u{00cd}', '\u{00ce}', '\u{010e}',
    '\u{0110}', '\u{0143}', '\u{0147}', '\u{00d3}', '\u{00d4}', '\u{0150}', '\u{00d6}', '\u{00d7}',
    '\u{0158}', '\u{016e}', '\u{00da}', '\u{0170}', '\u{00dc}', '\u{00dd}', '\u{0162}', '\u{00df}',
    '\u{0155}', '\u{00e1}', '\u{00e2}', '\u{0103}', '\u{00e4}', '\u{013a}', '\u{0107}', '\u{00e7}',
    '\u{010d}', '\u{00e9}', '\u{0119}', '\u{00eb}', '\u{011b}', '\u{00ed}', '\u{00ee}', '\u{010f}',
    '\u{0111}', '\u{0144}', '\u{0148}', '\u{00f3}', '\u{00f4}', '\u{0151}', '\u{00f6}', '\u{00f7}',
    '\u{0159}', '\u{016f}', '\u{00fa}', '\u{0171}', '\u{00fc}', '\u{00fd}', '\u{0163}', '\u{02d9}',
];

#[cfg(test)]
#[path = "charset_windows1250_tests.rs"]
mod tests;
