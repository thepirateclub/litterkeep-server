//! Типизированная MIME Content-Transfer-Encoding и декодирование body.

use super::{decode_mime_base64, quoted_printable};

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
        let token = text.trim_matches(|character| matches!(character, ' ' | '\t'));
        match (token.is_empty(), token.bytes().all(is_token_octet)) {
            (false, true) => {}
            _ => return Err("invalid Content-Transfer-Encoding token"),
        }
        Ok(match token.to_ascii_lowercase().as_str() {
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
                validate_line_data(body, LineDataProfile::SevenBit)?;
                Ok(body.to_vec())
            }
            Self::EightBit => {
                validate_line_data(body, LineDataProfile::EightBit)?;
                Ok(body.to_vec())
            }
            Self::Binary => Ok(body.to_vec()),
            Self::Base64 => decode_mime_base64(body),
            Self::QuotedPrintable => quoted_printable::decode(body),
            Self::Unsupported(_) => Err("unsupported Content-Transfer-Encoding"),
        }
    }
}

/// Проверяет RFC 2045 token: печатный ASCII без separators и tspecials.
fn is_token_octet(byte: u8) -> bool {
    match byte {
        0x21..=0x7e => !matches!(
            byte,
            b'(' | b')'
                | b'<'
                | b'>'
                | b'@'
                | b','
                | b';'
                | b':'
                | b'\\'
                | b'"'
                | b'/'
                | b'['
                | b']'
                | b'?'
                | b'='
        ),
        _ => false,
    }
}

/// Ограничения RFC 2045 для 7bit и 8bit MIME-данных.
#[derive(Clone, Copy)]
enum LineDataProfile {
    SevenBit,
    EightBit,
}

/// Проверяет CRLF, NUL, ширину строки и допустимый диапазон октетов за O(n).
fn validate_line_data(body: &[u8], profile: LineDataProfile) -> Result<(), &'static str> {
    let mut cursor = 0_usize;
    let mut line_octets = 0_usize;
    while let Some(byte) = body.get(cursor).copied() {
        match byte {
            0 => return Err("NUL in MIME line data"),
            b'\r' => {
                let Some(b'\n') = body.get(cursor.saturating_add(1)).copied() else {
                    return Err("bare CR in MIME line data");
                };
                cursor = cursor.saturating_add(2);
                line_octets = 0;
            }
            b'\n' => return Err("bare LF in MIME line data"),
            0x80..=u8::MAX => match profile {
                LineDataProfile::SevenBit => return Err("non-ASCII octet in 7bit MIME body"),
                LineDataProfile::EightBit => advance_line(&mut cursor, &mut line_octets)?,
            },
            _ => advance_line(&mut cursor, &mut line_octets)?,
        }
    }
    Ok(())
}

/// Добавляет один data octet к текущей MIME line, не превышая RFC предел.
fn advance_line(cursor: &mut usize, line_octets: &mut usize) -> Result<(), &'static str> {
    match line_octets.saturating_add(1) {
        0..=998 => {
            *line_octets = line_octets.saturating_add(1);
            *cursor = cursor.saturating_add(1);
            Ok(())
        }
        _ => Err("MIME line exceeds 998 octets"),
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

    /// RFC 2045 разрешает ASCII linear whitespace вокруг CTE token, но не Unicode space.
    #[test]
    fn parser_rejects_non_ascii_whitespace_and_non_token_octets() {
        assert_eq!(
            TransferEncoding::parse(b" \tBASE64\t "),
            Ok(TransferEncoding::Base64)
        );
        for invalid in [
            "base64\u{00a0}".as_bytes(),
            "\u{2003}base64".as_bytes(),
            b"base 64".as_slice(),
            b"base64/extra".as_slice(),
        ] {
            assert!(
                TransferEncoding::parse(invalid).is_err(),
                "accepted invalid CTE token {invalid:?}"
            );
        }
        assert_eq!(
            TransferEncoding::parse(b"x-future-cte"),
            Ok(TransferEncoding::Unsupported("x-future-cte".to_owned()))
        );
    }

    #[test]
    fn seven_bit_and_eight_bit_enforce_rfc_line_data_rules() {
        let legal_seven_bit = vec![b'a'; 998];
        assert_eq!(
            TransferEncoding::SevenBit.decode(&legal_seven_bit),
            Ok(legal_seven_bit)
        );
        assert!(TransferEncoding::SevenBit.decode(&vec![b'a'; 999]).is_err());
        assert!(TransferEncoding::SevenBit.decode(b"a\0b").is_err());
        assert!(TransferEncoding::SevenBit.decode(b"a\rb").is_err());
        assert!(TransferEncoding::SevenBit.decode(b"a\nb").is_err());

        assert_eq!(
            TransferEncoding::EightBit.decode(b"\xff\r\ntext"),
            Ok(b"\xff\r\ntext".to_vec())
        );
        assert!(TransferEncoding::EightBit.decode(b"\xff\0").is_err());
        assert!(TransferEncoding::EightBit.decode(&vec![b'x'; 999]).is_err());
    }
}
