//! Ограниченный и явный профиль преобразования MIME charset в Unicode.

/// Поддерживаемые charset после нормализации на протокольной границе.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Charset {
    UsAscii,
    Utf8,
    Iso8859_1,
    Windows1251,
}

impl Charset {
    /// Преобразует внешнее имя charset в enum, не распространяя строки дальше.
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        match name.to_ascii_lowercase().as_str() {
            "us-ascii" | "ascii" => Ok(Self::UsAscii),
            "utf-8" | "utf8" => Ok(Self::Utf8),
            "iso-8859-1" | "latin1" => Ok(Self::Iso8859_1),
            "windows-1251" | "cp1251" => Ok(Self::Windows1251),
            _ => Err("unsupported MIME charset"),
        }
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
            Self::Windows1251 => decode_windows_1251(bytes),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_explicit_and_strict() {
        assert_eq!(Charset::Utf8.decode("ёж".as_bytes()).expect("utf8"), "ёж");
        assert_eq!(
            Charset::Iso8859_1.decode(b"caf\xe9").expect("latin1"),
            "café"
        );
        assert!(Charset::UsAscii.decode(b"\xff").is_err());
        assert_eq!(
            Charset::parse("windows-1251")
                .expect("cp1251")
                .decode(b"\xcf\xf0\xe8\xe2\xe5\xf2"),
            Ok("Привет".to_owned())
        );
        assert!(Charset::Windows1251.decode(b"\x98").is_err());
    }
}
