//! Ограниченный и явный профиль преобразования MIME charset в Unicode.

/// Поддерживаемые charset после нормализации на протокольной границе.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Charset {
    UsAscii,
    Utf8,
    Iso8859_1,
}

impl Charset {
    /// Преобразует внешнее имя charset в enum, не распространяя строки дальше.
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        match name.to_ascii_lowercase().as_str() {
            "us-ascii" | "ascii" => Ok(Self::UsAscii),
            "utf-8" | "utf8" => Ok(Self::Utf8),
            "iso-8859-1" | "latin1" => Ok(Self::Iso8859_1),
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
        }
    }
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
        assert!(Charset::parse("windows-1251").is_err());
    }
}
