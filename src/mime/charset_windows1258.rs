//! Декодер MIME charset Windows-1258 с сохранением combining sequences.

/// Декодирует входные octets без NFC/NFD нормализации результата.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    bytes.iter().copied().map(decode_byte).collect()
}

/// Разделяет ASCII и два диапазона Microsoft mapping.
fn decode_byte(byte: u8) -> Result<char, &'static str> {
    match byte {
        0x00..=0x7f => Ok(char::from(byte)),
        0x80..=0x9f => lookup(&CONTROL_BLOCK, byte - 0x80),
        0xa0..=0xff => lookup(&GRAPHIC_BLOCK, byte - 0xa0),
    }
}

/// Читает только объявленные назначения, не индексируя массив оператором.
fn lookup(table: &[Option<char>], offset: u8) -> Result<char, &'static str> {
    table
        .get(usize::from(offset))
        .copied()
        .flatten()
        .ok_or("undefined Windows-1258 octet")
}

/// Назначения диапазона 80..9F; None сохраняет undefined positions.
const CONTROL_BLOCK: [Option<char>; 32] = [
    Some('€'),
    None,
    Some('‚'),
    Some('ƒ'),
    Some('„'),
    Some('…'),
    Some('†'),
    Some('‡'),
    Some('ˆ'),
    Some('‰'),
    None,
    Some('‹'),
    Some('Œ'),
    None,
    None,
    None,
    None,
    Some('‘'),
    Some('’'),
    Some('“'),
    Some('”'),
    Some('•'),
    Some('–'),
    Some('—'),
    Some('˜'),
    Some('™'),
    None,
    Some('›'),
    Some('œ'),
    None,
    None,
    Some('Ÿ'),
];

/// Назначения диапазона A0..FF, включая вьетнамские combining accents.
const GRAPHIC_BLOCK: [Option<char>; 96] = [
    Some('\u{00a0}'),
    Some('¡'),
    Some('¢'),
    Some('£'),
    Some('¤'),
    Some('¥'),
    Some('¦'),
    Some('§'),
    Some('¨'),
    Some('©'),
    Some('ª'),
    Some('«'),
    Some('¬'),
    Some('\u{00ad}'),
    Some('®'),
    Some('¯'),
    Some('°'),
    Some('±'),
    Some('²'),
    Some('³'),
    Some('´'),
    Some('µ'),
    Some('¶'),
    Some('·'),
    Some('¸'),
    Some('¹'),
    Some('º'),
    Some('»'),
    Some('¼'),
    Some('½'),
    Some('¾'),
    Some('¿'),
    Some('À'),
    Some('Á'),
    Some('Â'),
    Some('Ă'),
    Some('Ä'),
    Some('Å'),
    Some('Æ'),
    Some('Ç'),
    Some('È'),
    Some('É'),
    Some('Ê'),
    Some('Ë'),
    Some('\u{0300}'),
    Some('Í'),
    Some('Î'),
    Some('Ï'),
    Some('Đ'),
    Some('Ñ'),
    Some('\u{0309}'),
    Some('Ó'),
    Some('Ô'),
    Some('Ơ'),
    Some('Ö'),
    Some('×'),
    Some('Ø'),
    Some('Ù'),
    Some('Ú'),
    Some('Û'),
    Some('Ü'),
    Some('Ư'),
    Some('\u{0303}'),
    Some('ß'),
    Some('à'),
    Some('á'),
    Some('â'),
    Some('ă'),
    Some('ä'),
    Some('å'),
    Some('æ'),
    Some('ç'),
    Some('è'),
    Some('é'),
    Some('ê'),
    Some('ë'),
    Some('\u{0301}'),
    Some('í'),
    Some('î'),
    Some('ï'),
    Some('đ'),
    Some('ñ'),
    Some('\u{0323}'),
    Some('ó'),
    Some('ô'),
    Some('ơ'),
    Some('ö'),
    Some('÷'),
    Some('ø'),
    Some('ù'),
    Some('ú'),
    Some('û'),
    Some('ü'),
    Some('ư'),
    Some('₫'),
    Some('ÿ'),
];

#[cfg(test)]
#[path = "charset_windows1258_tests.rs"]
mod tests;
