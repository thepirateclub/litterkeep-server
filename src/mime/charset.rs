//! Ограниченный и явный профиль преобразования MIME charset в Unicode.

#[cfg(test)]
#[path = "charset_alias_tests.rs"]
mod alias_tests;
#[cfg(test)]
#[path = "charset_iso8859_bidi_tests.rs"]
mod bidi_tests;
#[cfg(test)]
#[path = "charset_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "charset_utf16_integration_tests.rs"]
mod utf16_tests;

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
    Iso8859_6Explicit,
    Iso8859_6Implicit,
    Iso8859_7,
    Iso8859_8,
    Iso8859_8Explicit,
    Iso8859_8Implicit,
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
    Big5,
    EucJp,
    EucKr,
    Iso2022Jp,
    ShiftJis,
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
            "utf-8" | "csutf8" => Ok(Self::Utf8),
            "iso-8859-1" | "iso_8859-1" | "latin1" | "l1" | "ibm819" | "cp819" | "csisolatin1" => {
                Ok(Self::Iso8859_1)
            }
            "iso-8859-2" | "iso_8859-2" | "latin2" | "l2" | "csisolatin2" => Ok(Self::Iso8859_2),
            "iso-8859-3" | "iso_8859-3" | "latin3" | "l3" | "csisolatin3" => Ok(Self::Iso8859_3),
            "iso-8859-4" | "iso_8859-4" | "latin4" | "l4" | "csisolatin4" => Ok(Self::Iso8859_4),
            "iso-8859-5" | "iso_8859-5" | "cyrillic" | "csisolatincyrillic" => Ok(Self::Iso8859_5),
            "iso-8859-6" | "iso_8859-6" | "ecma-114" | "asmo-708" | "arabic"
            | "csisolatinarabic" => Ok(Self::Iso8859_6),
            "csiso88596e" | "iso-8859-6-e" | "iso_8859-6-e" => Ok(Self::Iso8859_6Explicit),
            "csiso88596i" | "iso-8859-6-i" | "iso_8859-6-i" => Ok(Self::Iso8859_6Implicit),
            "iso-8859-7" | "iso_8859-7" | "elot_928" | "ecma-118" | "greek" | "greek8"
            | "csisolatingreek" => Ok(Self::Iso8859_7),
            "iso-8859-8" | "iso_8859-8" | "hebrew" | "csisolatinhebrew" => Ok(Self::Iso8859_8),
            "csiso88598e" | "iso-8859-8-e" | "iso_8859-8-e" => Ok(Self::Iso8859_8Explicit),
            "csiso88598i" | "iso-8859-8-i" | "iso_8859-8-i" => Ok(Self::Iso8859_8Implicit),
            "iso-8859-9" | "latin5" | "l5" => Ok(Self::Iso8859_9),
            "iso-8859-10" | "iso_8859-10" | "iso_8859-10:1992" | "latin6" | "l6" => {
                Ok(Self::Iso8859_10)
            }
            "tis-620" | "iso-8859-11" => Ok(Self::Tis620),
            "iso-8859-13" | "iso_8859-13" | "csiso885913" => Ok(Self::Iso8859_13),
            "iso-8859-14" | "latin8" | "l8" | "iso-celtic" | "csiso885914" => Ok(Self::Iso8859_14),
            "iso-8859-15" | "iso_8859-15" | "latin-9" | "csiso885915" => Ok(Self::Iso8859_15),
            "iso-8859-16" | "iso_8859-16:2001" | "iso_8859-16" | "latin10" | "l10"
            | "iso-ir-226" | "csiso885916" => Ok(Self::Iso8859_16),
            "windows-1250" | "cswindows1250" => Ok(Self::Windows1250),
            "windows-1251" | "cswindows1251" => Ok(Self::Windows1251),
            "windows-1252" | "cp1252" | "cswindows1252" => Ok(Self::Windows1252),
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
            "big5" | "big5-hkscs" | "cn-big5" | "csbig5" | "x-x-big5" => Ok(Self::Big5),
            "cseucpkdfmtjapanese" | "euc-jp" | "x-euc-jp" => Ok(Self::EucJp),
            "cseuckr" | "csksc56011987" | "euc-kr" | "iso-ir-149" | "korean" | "ks_c_5601-1987"
            | "ks_c_5601-1989" | "ksc5601" | "ksc_5601" | "windows-949" => Ok(Self::EucKr),
            "csiso2022jp" | "iso-2022-jp" => Ok(Self::Iso2022Jp),
            "csshiftjis" | "cswindows31j" | "ms932" | "ms_kanji" | "shift-jis" | "shift_jis"
            | "sjis" | "windows-31j" | "x-sjis" => Ok(Self::ShiftJis),
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
            Self::Iso8859_6Explicit | Self::Iso8859_6Implicit => {
                super::charset_iso8859_6::decode(bytes)
            }
            Self::Iso8859_7 => super::charset_iso8859_7::decode(bytes),
            Self::Iso8859_8 => super::charset_iso8859_8::decode(bytes),
            Self::Iso8859_8Explicit | Self::Iso8859_8Implicit => {
                super::charset_iso8859_8::decode(bytes)
            }
            Self::Iso8859_9 => Ok(super::charset_iso8859_9::decode(bytes)),
            Self::Iso8859_10 => super::charset_iso8859_10::decode(bytes),
            Self::Tis620 => super::charset_tis620::decode(bytes),
            Self::Iso8859_13 => super::charset_iso8859_13::decode(bytes),
            Self::Iso8859_14 => super::charset_iso8859_14::decode(bytes),
            Self::Iso8859_15 => Ok(super::charset_iso8859_15::decode(bytes)),
            Self::Iso8859_16 => Ok(super::charset_iso8859_16::decode(bytes)),
            Self::Windows1250 => super::charset_windows1250::decode(bytes),
            Self::Windows1251 => super::charset_windows1251_1252::decode_1251(bytes),
            Self::Windows1252 => Ok(super::charset_windows1251_1252::decode_1252(bytes)),
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
            Self::Big5 => super::charset_big5::decode(bytes),
            Self::EucJp => super::charset_euc_jp::decode(bytes),
            Self::EucKr => super::charset_euc_kr::decode(bytes),
            Self::Iso2022Jp => super::charset_iso2022_jp::decode(bytes),
            Self::ShiftJis => super::charset_shift_jis::decode(bytes),
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
