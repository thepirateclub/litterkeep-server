//! Декодирование одного RFC 2047 encoded-word без переписывания raw header.

use super::{charset::Charset, decode_base64};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferEncoding {
    Base64,
    Q,
}

/// Декодирует полную форму `=?charset?B|Q?encoded-text?=`.
pub fn decode(word: &str) -> Result<String, &'static str> {
    let inner = word
        .strip_prefix("=?")
        .and_then(|value| value.strip_suffix("?="))
        .ok_or("invalid encoded-word envelope")?;
    let mut sections = inner.splitn(3, '?');
    let charset = Charset::parse(sections.next().ok_or("missing encoded-word charset")?)?;
    let encoding = parse_encoding(sections.next().ok_or("missing encoded-word encoding")?)?;
    let encoded = sections.next().ok_or("missing encoded-word text")?;
    if encoded.contains('?') {
        return Err("question mark inside encoded-word text");
    }
    let bytes = match encoding {
        TransferEncoding::Base64 => decode_base64(encoded.as_bytes())?,
        TransferEncoding::Q => decode_q(encoded.as_bytes())?,
    };
    charset.decode(&bytes)
}

fn parse_encoding(value: &str) -> Result<TransferEncoding, &'static str> {
    match value.to_ascii_uppercase().as_str() {
        "B" => Ok(TransferEncoding::Base64),
        "Q" => Ok(TransferEncoding::Q),
        _ => Err("unsupported encoded-word transfer encoding"),
    }
}

fn decode_q(input: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut output = Vec::with_capacity(input.len());
    let mut cursor = 0_usize;
    while let Some(byte) = input.get(cursor).copied() {
        match byte {
            b'_' => output.push(b' '),
            b'=' => {
                let high = input.get(cursor.saturating_add(1)).copied().and_then(hex);
                let low = input.get(cursor.saturating_add(2)).copied().and_then(hex);
                let (Some(high), Some(low)) = (high, low) else {
                    return Err("invalid encoded-word Q escape");
                };
                output.push((high << 4) | low);
                cursor = cursor.saturating_add(2);
            }
            b'?' | b' ' | b'\t' => return Err("invalid encoded-word Q character"),
            _ => output.push(byte),
        }
        cursor = cursor.saturating_add(1);
    }
    Ok(output)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64_utf8_and_q_latin1() {
        assert_eq!(
            decode("=?UTF-8?B?0J/RgNC40LLQtdGC?=").expect("base64"),
            "Привет"
        );
        assert_eq!(
            decode("=?ISO-8859-1?Q?caf=E9_file?=").expect("q"),
            "café file"
        );
    }

    #[test]
    fn rejects_unknown_charset_and_bad_q_escape() {
        assert!(decode("=?windows-1251?B?0J/RgNC40LLQtdGC?=").is_err());
        assert!(decode("=?UTF-8?Q?bad=ZZ?=").is_err());
    }
}
