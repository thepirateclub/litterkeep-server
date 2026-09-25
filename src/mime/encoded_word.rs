//! Декодирование одного RFC 2047 encoded-word без переписывания raw header.

use super::{charset::Charset, decode_base64};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferEncoding {
    Base64,
    Q,
}

/// Декодирует полную форму `=?charset?B|Q?encoded-text?=`.
pub fn decode(word: &str) -> Result<String, &'static str> {
    if word.len() > 75 {
        return Err("encoded-word exceeds 75 characters");
    }
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

/// Декодирует encoded-word только в позиции отдельного слова RFC 2047 phrase.
///
/// Некорректное либо неподдерживаемое слово остаётся обычным текстом: RFC 2047
/// запрещает из-за него отклонять или скрывать всё сообщение.
pub fn decode_phrase(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut previous_encoded = false;
    for word in value.split_whitespace() {
        let decoded = decode(word);
        match decoded {
            Ok(decoded) => {
                if !output.is_empty() && !previous_encoded {
                    output.push(' ');
                }
                output.push_str(&decoded);
                previous_encoded = true;
            }
            Err(_) => {
                if !output.is_empty() {
                    output.push(' ');
                }
                output.push_str(word);
                previous_encoded = false;
            }
        }
    }
    output
}

/// Декодирует все encoded-word в неструктурированном header field.
pub fn decode_header(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0_usize;
    let mut previous_was_encoded = false;
    while let Some(relative_start) = value.get(cursor..).and_then(|tail| tail.find("=?")) {
        let start = cursor.saturating_add(relative_start);
        let prefix = value
            .get(cursor..start)
            .ok_or("invalid encoded-word prefix")?;
        let search_start = start.saturating_add(2);
        let Some(relative_end) = value.get(search_start..).and_then(|tail| tail.find("?=")) else {
            output.push_str(value.get(cursor..).ok_or("invalid header tail")?);
            return Ok(output);
        };
        let end = search_start.saturating_add(relative_end).saturating_add(2);
        let candidate = value.get(start..end).ok_or("invalid encoded-word range")?;
        match decode(candidate) {
            Ok(decoded) => {
                match (
                    previous_was_encoded,
                    prefix.chars().all(char::is_whitespace),
                ) {
                    (true, true) => {}
                    _ => output.push_str(prefix),
                }
                output.push_str(&decoded);
                previous_was_encoded = true;
                cursor = end;
            }
            Err(_) => {
                output.push_str(
                    value
                        .get(cursor..search_start)
                        .ok_or("invalid raw prefix")?,
                );
                previous_was_encoded = false;
                cursor = search_start;
            }
        }
    }
    output.push_str(value.get(cursor..).ok_or("invalid final header range")?);
    Ok(output)
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
        assert_eq!(
            decode("=?windows-1251?Q?=CF=F0=E8=E2=E5=F2?=").expect("cp1251"),
            "Привет"
        );
        assert!(decode("=?UTF-8?Q?bad=ZZ?=").is_err());
    }

    #[test]
    fn decodes_adjacent_words_and_preserves_plain_prefix() {
        let value = "Re: =?UTF-8?B?0J/RgNC40LLQtdGC?= =?ISO-8859-1?Q?_caf=E9?=";
        assert_eq!(decode_header(value).expect("header"), "Re: Привет café");
    }
}
