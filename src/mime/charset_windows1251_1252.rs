//! Табличное преобразование Windows-1251 и Windows-1252.

/// Преобразует байты Windows-1251 и отклоняет незаданный RFC mapping.
pub(super) fn decode_1251(bytes: &[u8]) -> Result<String, &'static str> {
    bytes
        .iter()
        .copied()
        .map(decode_windows_1251_byte)
        .collect()
}

/// Декодирует один октет Windows-1251 с точным отображением пунктуации.
fn decode_windows_1251_byte(byte: u8) -> Result<char, &'static str> {
    let decoded = match byte {
        0x00..=0x7f => char::from(byte),
        0xc0..=0xff => char::from_u32(0x0410 + u32::from(byte - 0xc0))
            .ok_or("invalid Windows-1251 code point")?,
        0x80 => 'Ђ',
        0x81 => 'Ѓ',
        0x82 => '‚',
        0x83 => 'ѓ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => '€',
        0x89 => '‰',
        0x8a => 'Љ',
        0x8b => '‹',
        0x8c => 'Њ',
        0x8d => 'Ќ',
        0x8e => 'Ћ',
        0x8f => 'Џ',
        0x90 => 'ђ',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => return Err("undefined Windows-1251 octet"),
        0x99 => '™',
        0x9a => 'љ',
        0x9b => '›',
        0x9c => 'њ',
        0x9d => 'ќ',
        0x9e => 'ћ',
        0x9f => 'џ',
        0xa0 => '\u{00a0}',
        0xa1 => 'Ў',
        0xa2 => 'ў',
        0xa3 => 'Ј',
        0xa4 => '¤',
        0xa5 => 'Ґ',
        0xa6 => '¦',
        0xa7 => '§',
        0xa8 => 'Ё',
        0xa9 => '©',
        0xaa => 'Є',
        0xab => '«',
        0xac => '¬',
        0xad => '\u{00ad}',
        0xae => '®',
        0xaf => 'Ї',
        0xb0 => '°',
        0xb1 => '±',
        0xb2 => 'І',
        0xb3 => 'і',
        0xb4 => 'ґ',
        0xb5 => 'µ',
        0xb6 => '¶',
        0xb7 => '·',
        0xb8 => 'ё',
        0xb9 => '№',
        0xba => 'є',
        0xbb => '»',
        0xbc => 'ј',
        0xbd => 'Ѕ',
        0xbe => 'ѕ',
        0xbf => 'ї',
    };
    Ok(decoded)
}

/// Декодирует массив байтов Windows-1252 по WHATWG mapping.
pub(super) fn decode_1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .map(decode_windows_1252_byte)
        .collect()
}

/// Преобразует один Windows-1252 octet без таблицы с входным индексом.
fn decode_windows_1252_byte(byte: u8) -> char {
    match byte {
        0x00..=0x7f | 0x81 | 0x8d | 0x8f | 0x90 | 0x9d | 0xa0..=0xff => char::from(byte),
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8a => 'Š',
        0x8b => '‹',
        0x8c => 'Œ',
        0x8e => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9a => 'š',
        0x9b => '›',
        0x9c => 'œ',
        0x9e => 'ž',
        0x9f => 'Ÿ',
    }
}
