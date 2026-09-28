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
    Iso8859_3,
    Iso8859_4,
    Iso8859_5,
    Iso8859_6,
    Iso8859_7,
    Iso8859_8,
    Iso8859_9,
    Iso8859_10,
    Tis620,
    Iso8859_13,
    Iso8859_14,
    Iso8859_15,
    Iso8859_16,
    Windows1250,
    Windows1251,
    Windows1252,
    Windows1253,
    Windows1254,
    Windows1255,
    Windows1256,
    Windows1257,
    Windows1258,
    Windows874,
    Ibm866,
    Koi8R,
    Koi8U,
    Gbk,
    Gb18030,
    Utf16,
    Utf16Be,
    Utf16Le,
    Utf32,
    Utf32Be,
    Utf32Le,
}

impl Charset {
    /// Преобразует внешнее имя charset в enum, не распространяя строки дальше.
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        match name.to_ascii_lowercase().as_str() {
            "us-ascii" | "ascii" => Ok(Self::UsAscii),
            "utf-8" | "utf8" => Ok(Self::Utf8),
            "iso-8859-1" | "latin1" => Ok(Self::Iso8859_1),
            "iso-8859-2" => Ok(Self::Iso8859_2),
            "iso-8859-3" => Ok(Self::Iso8859_3),
            "iso-8859-4" => Ok(Self::Iso8859_4),
            "iso-8859-5" => Ok(Self::Iso8859_5),
            "iso-8859-6" => Ok(Self::Iso8859_6),
            "iso-8859-7" => Ok(Self::Iso8859_7),
            "iso-8859-8" => Ok(Self::Iso8859_8),
            "iso-8859-9" | "latin5" | "l5" => Ok(Self::Iso8859_9),
            "iso-8859-10" | "iso_8859-10" | "iso_8859-10:1992" | "latin6" | "l6" => {
                Ok(Self::Iso8859_10)
            }
            "tis-620" | "iso-8859-11" => Ok(Self::Tis620),
            "iso-8859-13" | "csiso885913" => Ok(Self::Iso8859_13),
            "iso-8859-14" | "latin8" | "l8" | "iso-celtic" | "csiso885914" => Ok(Self::Iso8859_14),
            "iso-8859-15" | "iso_8859-15" | "latin-9" | "csiso885915" => Ok(Self::Iso8859_15),
            "iso-8859-16" | "iso_8859-16:2001" | "iso_8859-16" | "latin10" | "l10"
            | "iso-ir-226" | "csiso885916" => Ok(Self::Iso8859_16),
            "windows-1250" | "cswindows1250" => Ok(Self::Windows1250),
            "windows-1251" | "cp1251" => Ok(Self::Windows1251),
            "windows-1252" => Ok(Self::Windows1252),
            "windows-1253" | "cswindows1253" => Ok(Self::Windows1253),
            "windows-1254" | "cswindows1254" => Ok(Self::Windows1254),
            "windows-1255" | "cswindows1255" => Ok(Self::Windows1255),
            "windows-1256" | "cswindows1256" => Ok(Self::Windows1256),
            "windows-1257" | "cswindows1257" => Ok(Self::Windows1257),
            "windows-1258" | "cswindows1258" => Ok(Self::Windows1258),
            "windows-874" | "cswindows874" => Ok(Self::Windows874),
            "ibm866" | "cp866" | "csibm866" | "866" => Ok(Self::Ibm866),
            "koi8-r" | "cskoi8r" => Ok(Self::Koi8R),
            "koi8-u" | "cskoi8u" => Ok(Self::Koi8U),
            "gbk" | "gb2312" | "chinese" | "x-gbk" | "csgbk" | "cp936" | "ms936"
            | "windows-936" => Ok(Self::Gbk),
            "gb18030" | "csgb18030" => Ok(Self::Gb18030),
            "utf-16" => Ok(Self::Utf16),
            "utf-16be" => Ok(Self::Utf16Be),
            "utf-16le" => Ok(Self::Utf16Le),
            "utf-32" => Ok(Self::Utf32),
            "utf-32be" => Ok(Self::Utf32Be),
            "utf-32le" => Ok(Self::Utf32Le),
            _ => Err("unsupported MIME charset"),
        }
    }

    /// Проверяет допустимость charset именно для MIME-подтипа text.
    pub fn is_suitable_for_text_mime(self) -> bool {
        !matches!(
            self,
            Self::Utf16
                | Self::Utf16Be
                | Self::Utf16Le
                | Self::Utf32
                | Self::Utf32Be
                | Self::Utf32Le
        )
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
            Self::Iso8859_3 => super::charset_iso8859_3::decode(bytes),
            Self::Iso8859_4 => super::charset_iso8859_4::decode(bytes),
            Self::Iso8859_5 => Ok(super::charset_iso8859_5::decode(bytes)),
            Self::Iso8859_6 => super::charset_iso8859_6::decode(bytes),
            Self::Iso8859_7 => super::charset_iso8859_7::decode(bytes),
            Self::Iso8859_8 => super::charset_iso8859_8::decode(bytes),
            Self::Iso8859_9 => Ok(super::charset_iso8859_9::decode(bytes)),
            Self::Iso8859_10 => super::charset_iso8859_10::decode(bytes),
            Self::Tis620 => super::charset_tis620::decode(bytes),
            Self::Iso8859_13 => super::charset_iso8859_13::decode(bytes),
            Self::Iso8859_14 => super::charset_iso8859_14::decode(bytes),
            Self::Iso8859_15 => Ok(super::charset_iso8859_15::decode(bytes)),
            Self::Iso8859_16 => Ok(super::charset_iso8859_16::decode(bytes)),
            Self::Windows1250 => super::charset_windows1250::decode(bytes),
            Self::Windows1251 => decode_windows_1251(bytes),
            Self::Windows1252 => Ok(decode_windows_1252(bytes)),
            Self::Windows1253 => super::charset_windows1253::decode(bytes),
            Self::Windows1254 => super::charset_windows1254::decode(bytes),
            Self::Windows1255 => super::charset_windows1255::decode(bytes),
            Self::Windows1256 => super::charset_windows1256::decode(bytes),
            Self::Windows1257 => super::charset_windows1257::decode(bytes),
            Self::Windows1258 => super::charset_windows1258::decode(bytes),
            Self::Windows874 => super::charset_windows874::decode(bytes),
            Self::Ibm866 => Ok(super::charset_ibm866::decode(bytes)),
            Self::Koi8R => Ok(super::charset_koi8_r::decode(bytes)),
            Self::Koi8U => Ok(super::charset_koi8_u::decode(bytes)),
            Self::Gbk | Self::Gb18030 => super::charset_gb18030::decode(bytes),
            Self::Utf16 => {
                super::charset_utf16::decode(bytes, super::charset_utf16::ByteOrder::DetectBom)
            }
            Self::Utf16Be => {
                super::charset_utf16::decode(bytes, super::charset_utf16::ByteOrder::BigEndian)
            }
            Self::Utf16Le => {
                super::charset_utf16::decode(bytes, super::charset_utf16::ByteOrder::LittleEndian)
            }
            Self::Utf32 => {
                super::charset_utf32::decode(bytes, super::charset_utf32::ByteOrder::DetectBom)
            }
            Self::Utf32Be => {
                super::charset_utf32::decode(bytes, super::charset_utf32::ByteOrder::BigEndian)
            }
            Self::Utf32Le => {
                super::charset_utf32::decode(bytes, super::charset_utf32::ByteOrder::LittleEndian)
            }
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
