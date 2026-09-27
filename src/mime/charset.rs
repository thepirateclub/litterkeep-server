//! Ограниченный и явный профиль преобразования MIME charset в Unicode.

#[cfg(test)]
#[path = "charset_tests.rs"]
mod tests;

/// Поддерживаемые charset после нормализации на протокольной границе.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Charset {
    UsAscii,
    Utf8,
    Iso8859_1,
    Iso8859_2,
    Iso8859Ascii,
    Windows1251,
    Windows1252,
    Utf16,
    Utf16Be,
    Utf16Le,
}

impl Charset {
    /// Преобразует внешнее имя charset в enum, не распространяя строки дальше.
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        match name.to_ascii_lowercase().as_str() {
            "us-ascii" | "ascii" => Ok(Self::UsAscii),
            "utf-8" | "utf8" => Ok(Self::Utf8),
            "iso-8859-1" | "latin1" => Ok(Self::Iso8859_1),
            "iso-8859-2" => Ok(Self::Iso8859_2),
            // Остальные зарегистрированные предпочитаемые MIME labels имеют
            // общий ASCII mapping, но отдельные high-bit таблицы не реализованы.
            "iso-8859-3" | "iso-8859-4" | "iso-8859-5" | "iso-8859-6" | "iso-8859-7"
            | "iso-8859-8" | "iso-8859-9" | "iso-8859-10" | "iso-8859-11" | "iso-8859-13"
            | "iso-8859-14" | "iso-8859-15" | "iso-8859-16" => Ok(Self::Iso8859Ascii),
            "windows-1251" | "cp1251" => Ok(Self::Windows1251),
            "windows-1252" => Ok(Self::Windows1252),
            "utf-16" => Ok(Self::Utf16),
            "utf-16be" => Ok(Self::Utf16Be),
            "utf-16le" => Ok(Self::Utf16Le),
            _ => Err("unsupported MIME charset"),
        }
    }

    /// Проверяет допустимость charset именно для MIME-подтипа text.
    pub fn is_suitable_for_text_mime(self) -> bool {
        !matches!(self, Self::Utf16 | Self::Utf16Be | Self::Utf16Le)
    }

    /// Декодирует только известный профиль, сохраняя ошибку неизвестной кодировки.
    pub fn decode(self, bytes: &[u8]) -> Result<String, &'static str> {
        match self {
            Self::UsAscii => {
                if !bytes.is_ascii() {
                    return Err("non-ASCII octet in US-ASCII text");
                }
                String::from_utf8(bytes.to_vec()).map_err(|_| "invalid US-ASCII")
            }
            Self::Utf8 => std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| "invalid UTF-8"),
            Self::Iso8859_1 => Ok(bytes.iter().map(|byte| char::from(*byte)).collect()),
            Self::Iso8859_2 => super::charset_iso8859_2::decode(bytes),
            // Не угадываем национальную таблицу: преобразуем только общий ASCII subset.
            Self::Iso8859Ascii => {
                if !bytes.is_ascii() {
                    return Err("non-ASCII octet in ISO-8859 ASCII subset");
                }
                String::from_utf8(bytes.to_vec()).map_err(|_| "invalid ISO-8859 ASCII subset")
            }
            Self::Windows1251 => decode_windows_1251(bytes),
            Self::Windows1252 => Ok(decode_windows_1252(bytes)),
            Self::Utf16 => decode_utf16(bytes, Utf16ByteOrder::DetectBom),
            Self::Utf16Be => decode_utf16(bytes, Utf16ByteOrder::BigEndian),
            Self::Utf16Le => decode_utf16(bytes, Utf16ByteOrder::LittleEndian),
        }
    }
}

/// Порядок байтов UTF-16, заданный меткой или начальной сигнатурой.
#[derive(Clone, Copy)]
enum Utf16ByteOrder {
    DetectBom,
    BigEndian,
    LittleEndian,
}

/// Декодирует UTF-16 с проверкой длины, surrogate pairs и невозможного U+FFFE.
fn decode_utf16(bytes: &[u8], requested_order: Utf16ByteOrder) -> Result<String, &'static str> {
    let (order, payload) = match (requested_order, bytes.get(..2)) {
        (Utf16ByteOrder::DetectBom, None) => {
            return Err("UTF-16 requires two octets to determine byte order");
        }
        (Utf16ByteOrder::DetectBom, Some([0xfe, 0xff])) => (
            Utf16ByteOrder::BigEndian,
            bytes.get(2..).ok_or("invalid UTF-16 BOM")?,
        ),
        (Utf16ByteOrder::DetectBom, Some([0xff, 0xfe])) => (
            Utf16ByteOrder::LittleEndian,
            bytes.get(2..).ok_or("invalid UTF-16 BOM")?,
        ),
        (Utf16ByteOrder::DetectBom, _) => (Utf16ByteOrder::BigEndian, bytes),
        (order, _) => (order, bytes),
    };
    if !payload.chunks_exact(2).remainder().is_empty() {
        return Err("odd UTF-16 octet count");
    }
    let units = payload
        .chunks_exact(2)
        .map(|pair| utf16_unit(pair, order))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = String::with_capacity(payload.len());
    for decoded in char::decode_utf16(units) {
        output.push(decoded.map_err(|_| "invalid UTF-16 surrogate pair")?);
    }
    Ok(output)
}

/// Собирает один 16-битный unit из пары октетов без прямой индексации.
fn utf16_unit(pair: &[u8], order: Utf16ByteOrder) -> Result<u16, &'static str> {
    let octets: [u8; 2] = pair.try_into().map_err(|_| "invalid UTF-16 pair")?;
    let value = match order {
        Utf16ByteOrder::DetectBom | Utf16ByteOrder::BigEndian => u16::from_be_bytes(octets),
        Utf16ByteOrder::LittleEndian => u16::from_le_bytes(octets),
    };
    match value {
        0xfffe => Err("UTF-16 contains the invalid U+FFFE value"),
        _ => Ok(value),
    }
}

/// Преобразует байты Windows-1251 без таблиц с индексированием по входу.
fn decode_windows_1251(bytes: &[u8]) -> Result<String, &'static str> {
    bytes
        .iter()
        .copied()
        .map(decode_windows_1251_byte)
        .collect()
}

/// Декодирует один октет Windows-1251 и отклоняет незаданный RFC mapping.
fn decode_windows_1251_byte(byte: u8) -> Result<char, &'static str> {
    // В верхней четверти диапазона строчные и прописные русские буквы идут
    // последовательно; пунктуационная часть задаётся отдельными точными ветвями.
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
        // Спецификация оставляет этот октет незаданным, поэтому не подменяем его символом.
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

/// Декодирует Windows-1252 согласно web-compatible индексу WHATWG.
fn decode_windows_1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .map(decode_windows_1252_byte)
        .collect()
}

/// Преобразует один октет Windows-1252 без таблицы с прямой индексацией.
///
/// В отличие от ISO-8859-1, диапазон 0x80..0x9F содержит типографские знаки;
/// пять позиций остаются соответствующими C1 control code points.
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
