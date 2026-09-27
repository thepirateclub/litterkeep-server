//! Таблица преобразования MIME charset ISO-8859-8 (Hebrew, visual order).

/// Декодирует назначенные значения ISO-8859-8 и отклоняет undefined позиции.
///
/// Базовый MIME charset задаёт byte mapping, но не выполняет bidi layout.
/// ISO-8859-8-I/E — отдельные directional profiles, они здесь не смешиваются.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один байт безопасным доступом к upper-half mapping.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .flatten()
                .ok_or("undefined ISO-8859-8 octet")
        }
    }
}

/// Назначения A0..FF; undefined positions остаются `None`.
const UPPER_HALF: [Option<char>; 96] = [
    Some('\u{00a0}'),
    None,
    Some('\u{00a2}'),
    Some('\u{00a3}'),
    Some('\u{00a4}'),
    Some('\u{00a5}'),
    Some('\u{00a6}'),
    Some('\u{00a7}'),
    Some('\u{00a8}'),
    Some('\u{00a9}'),
    Some('\u{00d7}'),
    Some('\u{00ab}'),
    Some('\u{00ac}'),
    Some('\u{00ad}'),
    Some('\u{00ae}'),
    Some('\u{00af}'),
    Some('\u{00b0}'),
    Some('\u{00b1}'),
    Some('\u{00b2}'),
    Some('\u{00b3}'),
    Some('\u{00b4}'),
    Some('\u{00b5}'),
    Some('\u{00b6}'),
    Some('\u{00b7}'),
    Some('\u{00b8}'),
    Some('\u{00b9}'),
    Some('\u{00f7}'),
    Some('\u{00bb}'),
    Some('\u{00bc}'),
    Some('\u{00bd}'),
    Some('\u{00be}'),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some('\u{2017}'),
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
#[path = "charset_iso8859_8_tests.rs"]
mod tests;
