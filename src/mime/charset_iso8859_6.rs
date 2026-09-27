//! Таблица преобразования MIME charset ISO-8859-6 (арабский алфавит).

/// Декодирует назначенные ISO-8859-6 значения и отклоняет undefined octets.
///
/// Верхний диапазон этой таблицы разреженный. Явные `None` не позволяют
/// трактовать зарезервированные позиции как символы или как другую кодировку.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Преобразует один octet по общей нижней половине и арабской upper table.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x9f => Ok(char::from(byte)),
        0xa0..=0xff => {
            let offset = usize::from(byte.saturating_sub(0xa0));
            UPPER_HALF
                .get(offset)
                .copied()
                .flatten()
                .ok_or("undefined ISO-8859-6 octet")
        }
    }
}

/// Назначения A0..FF; пустые позиции в стандарте не отображаются на Unicode.
const UPPER_HALF: [Option<char>; 96] = [
    Some('\u{00a0}'),
    None,
    None,
    None,
    Some('\u{00a4}'),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some('\u{060c}'),
    Some('\u{00ad}'),
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
    Some('\u{061b}'),
    None,
    None,
    None,
    Some('\u{061f}'),
    None,
    Some('\u{0621}'),
    Some('\u{0622}'),
    Some('\u{0623}'),
    Some('\u{0624}'),
    Some('\u{0625}'),
    Some('\u{0626}'),
    Some('\u{0627}'),
    Some('\u{0628}'),
    Some('\u{0629}'),
    Some('\u{062a}'),
    Some('\u{062b}'),
    Some('\u{062c}'),
    Some('\u{062d}'),
    Some('\u{062e}'),
    Some('\u{062f}'),
    Some('\u{0630}'),
    Some('\u{0631}'),
    Some('\u{0632}'),
    Some('\u{0633}'),
    Some('\u{0634}'),
    Some('\u{0635}'),
    Some('\u{0636}'),
    Some('\u{0637}'),
    Some('\u{0638}'),
    Some('\u{0639}'),
    Some('\u{063a}'),
    None,
    None,
    None,
    None,
    None,
    Some('\u{0640}'),
    Some('\u{0641}'),
    Some('\u{0642}'),
    Some('\u{0643}'),
    Some('\u{0644}'),
    Some('\u{0645}'),
    Some('\u{0646}'),
    Some('\u{0647}'),
    Some('\u{0648}'),
    Some('\u{0649}'),
    Some('\u{064a}'),
    Some('\u{064b}'),
    Some('\u{064c}'),
    Some('\u{064d}'),
    Some('\u{064e}'),
    Some('\u{064f}'),
    Some('\u{0650}'),
    Some('\u{0651}'),
    Some('\u{0652}'),
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
];

#[cfg(test)]
#[path = "charset_iso8859_6_tests.rs"]
mod tests;
