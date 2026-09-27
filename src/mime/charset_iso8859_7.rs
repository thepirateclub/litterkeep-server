//! Таблица преобразования MIME charset ISO-8859-7 (современная греческая).

/// Декодирует назначенные ISO-8859-7 байты и отклоняет три undefined позиции.
///
/// Нижняя половина, включая C1 controls, сохраняется как одноимённые Unicode
/// значения; upper-half mapping соответствует современному профилю Greek.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один байт без прямой индексации массива.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .flatten()
                .ok_or("undefined ISO-8859-7 octet")
        }
    }
}

/// Назначения A0..FF; `None` обозначает undefined позиции AE, D2 и FF.
const UPPER_HALF: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{00a3}'),
    Some('\u{20ac}'),
    Some('\u{20af}'),
    Some('\u{00a6}'),
    Some('\u{00a7}'),
    Some('\u{00a8}'),
    Some('\u{00a9}'),
    Some('\u{037a}'),
    Some('\u{00ab}'),
    Some('\u{00ac}'),
    Some('\u{00ad}'),
    None,
    Some('\u{2015}'),
    Some('\u{00b0}'),
    Some('\u{00b1}'),
    Some('\u{00b2}'),
    Some('\u{00b3}'),
    Some('\u{0384}'),
    Some('\u{0385}'),
    Some('\u{0386}'),
    Some('\u{00b7}'),
    Some('\u{0388}'),
    Some('\u{0389}'),
    Some('\u{038a}'),
    Some('\u{00bb}'),
    Some('\u{038c}'),
    Some('\u{00bd}'),
    Some('\u{038e}'),
    Some('\u{038f}'),
    Some('\u{0390}'),
    Some('\u{0391}'),
    Some('\u{0392}'),
    Some('\u{0393}'),
    Some('\u{0394}'),
    Some('\u{0395}'),
    Some('\u{0396}'),
    Some('\u{0397}'),
    Some('\u{0398}'),
    Some('\u{0399}'),
    Some('\u{039a}'),
    Some('\u{039b}'),
    Some('\u{039c}'),
    Some('\u{039d}'),
    Some('\u{039e}'),
    Some('\u{039f}'),
    Some('\u{03a0}'),
    Some('\u{03a1}'),
    None,
    Some('\u{03a3}'),
    Some('\u{03a4}'),
    Some('\u{03a5}'),
    Some('\u{03a6}'),
    Some('\u{03a7}'),
    Some('\u{03a8}'),
    Some('\u{03a9}'),
    Some('\u{03aa}'),
    Some('\u{03ab}'),
    Some('\u{03ac}'),
    Some('\u{03ad}'),
    Some('\u{03ae}'),
    Some('\u{03af}'),
    Some('\u{03b0}'),
    Some('\u{03b1}'),
    Some('\u{03b2}'),
    Some('\u{03b3}'),
    Some('\u{03b4}'),
    Some('\u{03b5}'),
    Some('\u{03b6}'),
    Some('\u{03b7}'),
    Some('\u{03b8}'),
    Some('\u{03b9}'),
    Some('\u{03ba}'),
    Some('\u{03bb}'),
    Some('\u{03bc}'),
    Some('\u{03bd}'),
    Some('\u{03be}'),
    Some('\u{03bf}'),
    Some('\u{03c0}'),
    Some('\u{03c1}'),
    Some('\u{03c2}'),
    Some('\u{03c3}'),
    Some('\u{03c4}'),
    Some('\u{03c5}'),
    Some('\u{03c6}'),
    Some('\u{03c7}'),
    Some('\u{03c8}'),
    Some('\u{03c9}'),
    Some('\u{03ca}'),
    Some('\u{03cb}'),
    Some('\u{03cc}'),
    Some('\u{03cd}'),
    Some('\u{03ce}'),
    None,
];

#[cfg(test)]
#[path = "charset_iso8859_7_tests.rs"]
mod tests;
