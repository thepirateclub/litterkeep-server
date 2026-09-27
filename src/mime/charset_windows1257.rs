//! Декодер MIME charset Windows-1257 для балтийских языков.

/// Преобразует сообщение по таблице Microsoft, не заполняя undefined octets.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Разделяет ASCII, control extension и Baltic graphic assignments.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => lookup(&CONTROL_BLOCK, byte - 0x80),
        0xa0..=0xff => lookup(&GRAPHIC_BLOCK, byte - 0xa0),
    }
}

/// Читает ограниченное смещение без оператора прямого индексирования.
fn lookup(table: &[Option<char>], offset: u8) -> Result<char, &'static str> {
    table
        .get(usize::from(offset))
        .copied()
        .flatten()
        .ok_or("undefined Windows-1257 octet")
}

/// Назначения 80..9F по Microsoft CP1257.
const CONTROL_BLOCK: [Option<char>; 32] = [
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
    None,
    Some('‹'),
    None,
    Some('¨'),
    Some('ˇ'),
    Some('¸'),
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
    None,
    Some('›'),
    None,
    Some('¯'),
    Some('˛'),
    None,
];

/// Назначения A0..FF для валют, пунктуации и Baltic букв.
const GRAPHIC_BLOCK: [Option<char>; 96] = [
    Some('\u{00a0}'),
    None,
    Some('¢'),
    Some('£'),
    Some('¤'),
    None,
    Some('¦'),
    Some('§'),
    Some('Ø'),
    Some('©'),
    Some('Ŗ'),
    Some('«'),
    Some('¬'),
    Some('\u{00ad}'),
    Some('®'),
    Some('Æ'),
    Some('°'),
    Some('±'),
    Some('²'),
    Some('³'),
    Some('´'),
    Some('µ'),
    Some('¶'),
    Some('·'),
    Some('ø'),
    Some('¹'),
    Some('ŗ'),
    Some('»'),
    Some('¼'),
    Some('½'),
    Some('¾'),
    Some('æ'),
    Some('Ą'),
    Some('Į'),
    Some('Ā'),
    Some('Ć'),
    Some('Ä'),
    Some('Å'),
    Some('Ę'),
    Some('Ē'),
    Some('Č'),
    Some('É'),
    Some('Ź'),
    Some('Ė'),
    Some('Ģ'),
    Some('Ķ'),
    Some('Ī'),
    Some('Ļ'),
    Some('Š'),
    Some('Ń'),
    Some('Ņ'),
    Some('Ó'),
    Some('Ō'),
    Some('Õ'),
    Some('Ö'),
    Some('×'),
    Some('Ų'),
    Some('Ł'),
    Some('Ś'),
    Some('Ū'),
    Some('Ü'),
    Some('Ż'),
    Some('Ž'),
    Some('ß'),
    Some('ą'),
    Some('į'),
    Some('ā'),
    Some('ć'),
    Some('ä'),
    Some('å'),
    Some('ę'),
    Some('ē'),
    Some('č'),
    Some('é'),
    Some('ź'),
    Some('ė'),
    Some('ģ'),
    Some('ķ'),
    Some('ī'),
    Some('ļ'),
    Some('š'),
    Some('ń'),
    Some('ņ'),
    Some('ó'),
    Some('ō'),
    Some('õ'),
    Some('ö'),
    Some('÷'),
    Some('ų'),
    Some('ł'),
    Some('ś'),
    Some('ū'),
    Some('ü'),
    Some('ż'),
    Some('ž'),
    Some('˙'),
];

#[cfg(test)]
#[path = "charset_windows1257_tests.rs"]
mod tests;
