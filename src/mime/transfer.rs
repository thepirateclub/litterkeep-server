//! Типизированная MIME Content-Transfer-Encoding и декодирование body.

use super::{decode_base64, quoted_printable};

/// Поддерживаемые transfer encoding после wire-разбора.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferEncoding {
    SevenBit,
    EightBit,
    Binary,
    Base64,
    QuotedPrintable,
    Unsupported(String),
}

impl TransferEncoding {
    /// Каноническое имя для IMAP BODYSTRUCTURE.
    pub fn wire_name(&self) -> &str {
        match self {
            Self::SevenBit => "7BIT",
            Self::EightBit => "8BIT",
            Self::Binary => "BINARY",
            Self::Base64 => "BASE64",
            Self::QuotedPrintable => "QUOTED-PRINTABLE",
            Self::Unsupported(value) => value,
        }
    }

    /// Преобразует внешнее имя в enum один раз на границе MIME parser-а.
    pub fn parse(value: &[u8]) -> Result<Self, &'static str> {
        let text = std::str::from_utf8(value).map_err(|_| "transfer encoding is not ASCII")?;
        Ok(match text.trim().to_ascii_lowercase().as_str() {
            "7bit" => Self::SevenBit,
            "8bit" => Self::EightBit,
            "binary" => Self::Binary,
            "base64" => Self::Base64,
            "quoted-printable" => Self::QuotedPrintable,
            other => Self::Unsupported(other.to_owned()),
        })
    }

    /// Возвращает декодированные octets либо явную ошибку неизвестной кодировки.
    pub fn decode(&self, body: &[u8]) -> Result<Vec<u8>, &'static str> {
        match self {
            Self::SevenBit => {
                if !body.is_ascii() {
                    return Err("non-ASCII octet in 7bit MIME body");
                }
                Ok(body.to_vec())
            }
            Self::EightBit | Self::Binary => Ok(body.to_vec()),
            Self::Base64 => decode_base64(body),
            Self::QuotedPrintable => quoted_printable::decode(body),
            Self::Unsupported(_) => Err("unsupported Content-Transfer-Encoding"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_is_selected_by_typed_encoding() {
        assert_eq!(
            TransferEncoding::Base64
                .decode(b"aGVsbG8=")
                .expect("base64"),
            b"hello"
        );
        assert_eq!(
            TransferEncoding::QuotedPrintable
                .decode(b"caf=C3=A9")
                .expect("quoted printable"),
            "café".as_bytes()
        );
        assert!(TransferEncoding::SevenBit.decode(b"\xff").is_err());
    }
}
